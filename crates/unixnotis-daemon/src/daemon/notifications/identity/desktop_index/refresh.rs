//! Debounced desktop-index refresh with atomic snapshot replacement

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use notify::event::{CreateKind, RemoveKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::model::DesktopIdentityIndex;

const REFRESH_DEBOUNCE: Duration = Duration::from_millis(500);
const MIN_REBUILD_INTERVAL: Duration = Duration::from_secs(5);
const REFRESH_SIGNAL_CAPACITY: usize = 1;
const MAX_WATCHED_DIRECTORIES: usize = 4_096;
const FALLBACK_REBUILD_INTERVAL: Duration = Duration::from_secs(90);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefreshTrigger {
    Filesystem,
    Fallback,
    Manual,
    WatchError,
    RecoveryVerification,
}

#[derive(Debug, Default)]
struct WatcherHealth {
    degraded: AtomicBool,
    installed: AtomicBool,
}

impl WatcherHealth {
    fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Acquire)
    }

    fn set_installed(&self, installed: bool) {
        self.installed.store(installed, Ordering::Release);
    }

    /// Returns true only for the first error from an installed watcher
    fn record_error(&self) -> bool {
        let first_error = !self.degraded.swap(true, Ordering::AcqRel);
        first_error && self.installed.load(Ordering::Acquire)
    }

    fn accepts_events(&self) -> bool {
        self.installed.load(Ordering::Acquire)
    }
}

struct WatcherInstance<W> {
    monitor: W,
    active_watches: HashSet<PathBuf>,
    health: Arc<WatcherHealth>,
}

type DesktopWatcherInstance = WatcherInstance<RecommendedWatcher>;

#[derive(Clone)]
pub struct DesktopIndexRefreshHandle {
    refresh_tx: mpsc::Sender<RefreshTrigger>,
}

impl DesktopIndexRefreshHandle {
    pub(crate) fn request_manual(&self) -> bool {
        match self.refresh_tx.try_send(RefreshTrigger::Manual) {
            Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => true,
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }
}

pub fn spawn_desktop_index_refresh(
    index: Arc<ArcSwap<DesktopIdentityIndex>>,
    watched_directories: Vec<PathBuf>,
) -> Result<DesktopIndexRefreshHandle> {
    let (refresh_tx, refresh_rx) = mpsc::channel(REFRESH_SIGNAL_CAPACITY);
    let requested_watches = watched_directories
        .into_iter()
        .take(MAX_WATCHED_DIRECTORIES)
        .collect::<HashSet<_>>();
    let watcher = create_watcher_instance(refresh_tx.clone(), &requested_watches)?;
    watcher.health.set_installed(true);
    let watch_coverage_incomplete =
        has_incomplete_watch_coverage(&requested_watches, &watcher.active_watches);
    if watch_coverage_incomplete {
        warn!(
            requested = requested_watches.len(),
            active = watcher.active_watches.len(),
            "desktop application watch coverage is incomplete; periodic rebuilds enabled"
        );
    }
    let worker_refresh_tx = refresh_tx.clone();

    tokio::spawn(run_refresh_worker(
        index,
        refresh_rx,
        watcher,
        worker_refresh_tx,
        watch_coverage_incomplete,
    ));

    Ok(DesktopIndexRefreshHandle { refresh_tx })
}

async fn run_refresh_worker(
    index: Arc<ArcSwap<DesktopIdentityIndex>>,
    mut refresh_rx: mpsc::Receiver<RefreshTrigger>,
    mut watcher: DesktopWatcherInstance,
    worker_refresh_tx: mpsc::Sender<RefreshTrigger>,
    mut watch_coverage_incomplete: bool,
) {
    // The watcher stays owned by this task for kernel watches to remain registered
    let mut fallback_tick =
        fallback_required(watch_coverage_incomplete, watcher.health.is_degraded())
            .then(fallback_interval);
    let mut last_rebuild = Instant::now()
        .checked_sub(MIN_REBUILD_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        let refresh_trigger = match fallback_tick.as_mut() {
            Some(tick) => tokio::select! {
                signal = refresh_rx.recv() => signal,
                _ = tick.tick() => Some(RefreshTrigger::Fallback),
            },
            None => refresh_rx.recv().await,
        };
        let Some(refresh_trigger) = refresh_trigger else {
            break;
        };
        update_fallback_timer(
            &mut fallback_tick,
            fallback_required(watch_coverage_incomplete, watcher.health.is_degraded()),
        );
        debug!(?refresh_trigger, "desktop application refresh requested");
        tokio::time::sleep(REFRESH_DEBOUNCE).await;
        // Drain events that arrived during the debounce window before one complete rebuild
        while refresh_rx.try_recv().is_ok() {}

        // Sustained user filesystem activity cannot trigger continuous complete rescans
        let remaining = rebuild_delay(last_rebuild.elapsed());
        tokio::time::sleep(remaining).await;
        match tokio::task::spawn_blocking(DesktopIdentityIndex::build_snapshot).await {
            Ok(rebuilt) => {
                let requested = rebuilt
                    .watched_directories
                    .into_iter()
                    .take(MAX_WATCHED_DIRECTORIES)
                    .collect::<HashSet<_>>();
                let mut watcher_recovered = false;
                if watcher.health.is_degraded() {
                    match create_watcher_instance(worker_refresh_tx.clone(), &requested) {
                        Ok(candidate) => {
                            watcher_recovered =
                                install_healthy_replacement(&mut watcher, candidate, &requested);
                            if watcher_recovered {
                                debug!(
                                    watched = watcher.active_watches.len(),
                                    "desktop application watcher reconstructed"
                                );
                                queue_recovery_verification(&worker_refresh_tx);
                            } else {
                                warn!(
                                    requested = requested.len(),
                                    "replacement desktop watcher was not healthy; retaining degraded watcher and periodic fallback"
                                );
                            }
                        }
                        Err(error) => {
                            warn!(?error, "failed to construct replacement desktop watcher");
                        }
                    }
                }

                if !watcher_recovered {
                    let additions = requested.difference(&watcher.active_watches).cloned();
                    let added = add_watch_directories(&mut watcher.monitor, additions);
                    watcher.active_watches.extend(added);
                }
                index.store(Arc::new(rebuilt.index));
                if !watcher_recovered {
                    remove_stale_watches(&mut watcher.monitor, &watcher.active_watches, &requested);
                    watcher
                        .active_watches
                        .retain(|directory| requested.contains(directory));
                }
                watch_coverage_incomplete =
                    has_incomplete_watch_coverage(&requested, &watcher.active_watches);
                update_fallback_timer(
                    &mut fallback_tick,
                    fallback_required(watch_coverage_incomplete, watcher.health.is_degraded()),
                );
                last_rebuild = Instant::now();
                debug!("desktop application identity index refreshed");
            }
            Err(error) => {
                warn!(?error, "desktop application identity index rebuild failed");
            }
        }
    }
}

fn create_watcher_instance(
    refresh_tx: mpsc::Sender<RefreshTrigger>,
    requested: &HashSet<PathBuf>,
) -> Result<DesktopWatcherInstance> {
    let health = Arc::new(WatcherHealth::default());
    let callback_health = Arc::clone(&health);
    let mut monitor = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        queue_refresh_event(event, &refresh_tx, &callback_health);
    })
    .context("create desktop application watcher")?;
    let active_watches = add_watch_directories(&mut monitor, requested.iter().cloned());
    if !registration_is_complete(requested, &active_watches) {
        health.degraded.store(true, Ordering::Release);
    }
    Ok(WatcherInstance {
        monitor,
        active_watches,
        health,
    })
}

fn has_incomplete_watch_coverage(requested: &HashSet<PathBuf>, active: &HashSet<PathBuf>) -> bool {
    requested.is_empty() || requested != active
}

fn registration_is_complete(requested: &HashSet<PathBuf>, active: &HashSet<PathBuf>) -> bool {
    requested == active
}

const fn fallback_required(watch_coverage_incomplete: bool, watcher_degraded: bool) -> bool {
    watch_coverage_incomplete || watcher_degraded
}

fn fallback_interval() -> tokio::time::Interval {
    tokio::time::interval_at(
        tokio::time::Instant::now() + FALLBACK_REBUILD_INTERVAL,
        FALLBACK_REBUILD_INTERVAL,
    )
}

fn update_fallback_timer(fallback_tick: &mut Option<tokio::time::Interval>, required: bool) {
    match (required, fallback_tick.is_some()) {
        (true, false) => *fallback_tick = Some(fallback_interval()),
        (false, true) => *fallback_tick = None,
        _ => {}
    }
}

const fn rebuild_delay(elapsed: Duration) -> Duration {
    MIN_REBUILD_INTERVAL.saturating_sub(elapsed)
}

fn queue_refresh_event(
    event: notify::Result<Event>,
    refresh_tx: &mpsc::Sender<RefreshTrigger>,
    health: &WatcherHealth,
) {
    match event {
        Ok(event) if health.accepts_events() && relevant_desktop_event(&event) => {
            // A single pending signal coalesces filesystem bursts without blocking the watcher
            let _ = refresh_tx.try_send(RefreshTrigger::Filesystem);
        }
        Ok(_) => {}
        Err(error) => {
            warn!(?error, "desktop application watcher reported an error");
            // Setup errors mark only the candidate; installed errors wake the worker once
            if health.record_error() {
                let _ = refresh_tx.try_send(RefreshTrigger::WatchError);
            }
        }
    }
}

fn queue_recovery_verification(refresh_tx: &mpsc::Sender<RefreshTrigger>) {
    let _ = refresh_tx.try_send(RefreshTrigger::RecoveryVerification);
}

fn install_healthy_replacement<W>(
    current: &mut WatcherInstance<W>,
    candidate: WatcherInstance<W>,
    requested: &HashSet<PathBuf>,
) -> bool {
    if !registration_is_complete(requested, &candidate.active_watches)
        || candidate.health.is_degraded()
    {
        return false;
    }
    // The candidate may receive events before the old instance is dropped
    candidate.health.set_installed(true);
    current.health.set_installed(false);
    *current = candidate;
    true
}

fn relevant_desktop_event(event: &Event) -> bool {
    // Folder changes alter the bounded nonrecursive watch set
    let folder_event = matches!(
        event.kind,
        EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder)
    );
    folder_event
        || event.paths.iter().any(|path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("desktop")
                || path.is_dir()
        })
}

fn add_watch_directories<W, I>(file_monitor: &mut W, directories: I) -> HashSet<PathBuf>
where
    W: Watcher,
    I: IntoIterator<Item = PathBuf>,
{
    let mut registered = HashSet::new();
    let mut failed = 0_usize;
    for directory in directories {
        if file_monitor
            .watch(Path::new(&directory), RecursiveMode::NonRecursive)
            .is_ok()
        {
            registered.insert(directory);
        } else {
            failed += 1;
        }
    }
    if failed != 0 {
        // One bounded summary avoids attacker-controlled path and error log floods
        warn!(
            failed,
            "some desktop application directories could not be watched"
        );
    }
    registered
}

fn remove_stale_watches<W>(
    file_monitor: &mut W,
    active: &HashSet<PathBuf>,
    requested: &HashSet<PathBuf>,
) where
    W: Watcher,
{
    for directory in active.difference(requested) {
        let _ = file_monitor.unwatch(directory);
    }
}

#[cfg(test)]
#[path = "tests/refresh.rs"]
mod tests;
