//! Debounced desktop-index refresh with atomic snapshot replacement

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use notify::event::{CreateKind, RemoveKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};
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
}

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
    let (refresh_tx, mut refresh_rx) = mpsc::channel(REFRESH_SIGNAL_CAPACITY);
    let watcher_tx = refresh_tx.clone();
    let mut file_monitor =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            queue_refresh_event(event, &watcher_tx);
        })
        .context("create desktop application watcher")?;

    let requested_watches = watched_directories
        .into_iter()
        .take(MAX_WATCHED_DIRECTORIES)
        .collect::<HashSet<_>>();
    let active_watches =
        add_watch_directories(&mut file_monitor, requested_watches.iter().cloned());
    let mut watch_coverage_incomplete =
        has_incomplete_watch_coverage(requested_watches.len(), active_watches.len());
    if watch_coverage_incomplete {
        warn!(
            requested = requested_watches.len(),
            active = active_watches.len(),
            "desktop application watch coverage is incomplete; periodic rebuilds enabled"
        );
    }

    tokio::spawn(async move {
        // The watcher must stay owned by this task for kernel watches to remain registered
        let mut file_monitor = file_monitor;
        let mut active_watches = active_watches;
        let mut fallback_tick = watch_coverage_incomplete.then(|| {
            // Missing watches include an empty set so a newly created directory is discovered
            tokio::time::interval_at(
                tokio::time::Instant::now() + FALLBACK_REBUILD_INTERVAL,
                FALLBACK_REBUILD_INTERVAL,
            )
        });
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
            if refresh_trigger == RefreshTrigger::WatchError && fallback_tick.is_none() {
                // A watcher error disables event coverage until a later rebuild succeeds
                fallback_tick = Some(tokio::time::interval_at(
                    tokio::time::Instant::now() + FALLBACK_REBUILD_INTERVAL,
                    FALLBACK_REBUILD_INTERVAL,
                ));
            }
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
                    // Add replacement watches before publishing the new immutable index
                    let additions = requested.difference(&active_watches).cloned();
                    let added = add_watch_directories(&mut file_monitor, additions);
                    index.store(Arc::new(rebuilt.index));
                    remove_stale_watches(&mut file_monitor, &active_watches, &requested);
                    active_watches.retain(|directory| requested.contains(directory));
                    active_watches.extend(added);
                    watch_coverage_incomplete =
                        has_incomplete_watch_coverage(requested.len(), active_watches.len());
                    if watch_coverage_incomplete && fallback_tick.is_none() {
                        // Continue polling after a rebuild if the watcher still misses paths
                        fallback_tick = Some(tokio::time::interval_at(
                            tokio::time::Instant::now() + FALLBACK_REBUILD_INTERVAL,
                            FALLBACK_REBUILD_INTERVAL,
                        ));
                    } else if !watch_coverage_incomplete {
                        fallback_tick = None;
                    }
                    last_rebuild = Instant::now();
                    debug!("desktop application identity index refreshed");
                }
                Err(error) => {
                    warn!(?error, "desktop application identity index rebuild failed");
                }
            }
        }
    });

    Ok(DesktopIndexRefreshHandle { refresh_tx })
}

const fn has_incomplete_watch_coverage(requested: usize, active: usize) -> bool {
    // An empty watch set can become valid later when an application directory appears
    requested == 0 || requested != active
}

const fn rebuild_delay(elapsed: Duration) -> Duration {
    MIN_REBUILD_INTERVAL.saturating_sub(elapsed)
}

fn queue_refresh_event(event: notify::Result<Event>, refresh_tx: &mpsc::Sender<RefreshTrigger>) {
    match event {
        Ok(event) if relevant_desktop_event(&event) => {
            // A single pending signal coalesces filesystem bursts without blocking the watcher
            let _ = refresh_tx.try_send(RefreshTrigger::Filesystem);
        }
        Ok(_) => {}
        Err(error) => {
            warn!(?error, "desktop application watcher reported an error");
            let _ = refresh_tx.try_send(RefreshTrigger::WatchError);
        }
    }
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
