//! D-Bus runtime for center UI events and control commands.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use unixnotis_core::{
    CloseReason, ControlProxy, ControlState, Margins, NotificationView, PanelDebugLevel,
    PanelRequest,
};
use zbus::{Connection, Result as ZbusResult};

// Backoff settings throttle reconnect attempts while keeping recovery responsive.
const BACKOFF_BASE_MS: u64 = 250;
const BACKOFF_MAX_MS: u64 = 5000;
const BACKOFF_JITTER_MS: u64 = 120;
// Retry warnings are rate-limited to avoid noisy logs during long outages.
const RETRY_WARN_INTERVAL_SECS: u64 = 30;
// Seed retries tolerate short startup hiccups without blocking indefinitely.
const SEED_RETRY_BASE_MS: u64 = 250;
const SEED_RETRY_MAX_MS: u64 = 2000;
const SEED_RETRY_BUDGET_SECS: u64 = 30;
const SEED_RETRY_LOG_INTERVAL_SECS: u64 = 10;
// Bound UI command queue to prevent unbounded growth during stalls.
const UI_COMMAND_QUEUE_CAPACITY: usize = 64;

struct Backoff {
    base: Duration,
    current: Duration,
    max: Duration,
}

impl Backoff {
    fn new(base_ms: u64, max_ms: u64) -> Self {
        let base = Duration::from_millis(base_ms);
        Self {
            base,
            current: base,
            max: Duration::from_millis(max_ms),
        }
    }

    fn reset(&mut self) {
        self.current = self.base;
    }

    fn next_sleep(&mut self) -> Duration {
        let jitter = jitter_duration(BACKOFF_JITTER_MS);
        let sleep = self.current;
        self.current = (self.current * 2).min(self.max);
        sleep + jitter
    }
}

fn jitter_duration(max_ms: u64) -> Duration {
    if max_ms == 0 {
        return Duration::from_millis(0);
    }
    // Simple xorshift-based jitter avoids deterministic alignment without extra dependencies.
    let jitter_ms = next_jitter_seed().wrapping_rem(max_ms);
    Duration::from_millis(jitter_ms)
}

fn next_jitter_seed() -> u64 {
    static STATE: AtomicU64 = AtomicU64::new(0);
    // Seed from wall clock once; subsequent calls evolve the state.
    let seed = STATE.load(Ordering::Relaxed);
    let mut value = if seed == 0 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        // Avoid a zero seed to keep the xorshift cycle moving.
        nanos | 1
    } else {
        seed
    };
    // xorshift64* variant for compact, fast jitter values.
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    value = value.wrapping_mul(0x2545F4914F6CDD1D);
    STATE.store(value, Ordering::Relaxed);
    value
}

// Rate-limited logger used to avoid warning spam during retry loops.
struct RetryLog {
    interval: Duration,
    last_warn: Instant,
}

impl RetryLog {
    fn new(interval: Duration) -> Self {
        let mut log = Self {
            interval,
            last_warn: Instant::now(),
        };
        log.reset();
        log
    }

    fn reset(&mut self) {
        // Allow the next failure after a success to emit a warning immediately.
        self.last_warn = Instant::now() - self.interval;
    }

    fn warn_or_debug<E: std::fmt::Debug>(&mut self, err: &E, message: &str) {
        if self.last_warn.elapsed() >= self.interval {
            self.last_warn = Instant::now();
            warn!(?err, "{message}");
        } else {
            debug!(?err, "{message}");
        }
    }

    fn warn_or_debug_seed(&mut self, err: &SeedError, message: &str) {
        if self.last_warn.elapsed() >= self.interval {
            self.last_warn = Instant::now();
            warn!(
                state_error = ?err.state_error,
                active_error = ?err.active_error,
                history_error = ?err.history_error,
                "{message}"
            );
        } else {
            debug!(
                state_error = ?err.state_error,
                active_error = ?err.active_error,
                history_error = ?err.history_error,
                "{message}"
            );
        }
    }
}

// Captures seed failures without forcing an immediate reconnect.
#[derive(Debug)]
struct SeedError {
    state_error: Option<String>,
    active_error: Option<String>,
    history_error: Option<String>,
}

use crate::debug;
use crate::media::MediaInfo;

/// Events delivered to the GTK main loop.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Seed {
        state: ControlState,
        active: Vec<NotificationView>,
        history: Vec<NotificationView>,
    },
    NotificationAdded(NotificationView, bool),
    NotificationUpdated(NotificationView, bool),
    NotificationClosed(u32, CloseReason),
    StateChanged(ControlState),
    PanelRequested(PanelRequest),
    GroupToggled(String),
    /// Updated set of active media players for the widget.
    MediaUpdated(Vec<MediaInfo>),
    MediaCleared,
    /// Hyprland active-window change that may indicate a click-away.
    ClickOutside,
    /// Hyprland reserved work area update for panel sizing.
    WorkAreaUpdated(Option<Margins>),
    RefreshWidgets,
    CssReload,
    ConfigReload,
}

/// Commands sent from GTK handlers to the D-Bus runtime.
#[derive(Debug, Clone)]
pub enum UiCommand {
    Dismiss(u32),
    InvokeAction { id: u32, action_key: String },
    ClearAll,
    SetDnd(bool),
    ClosePanel,
}

pub fn start_dbus_task(
    runtime: &tokio::runtime::Handle,
    connection: Connection,
    sender: async_channel::Sender<UiEvent>,
) -> mpsc::Sender<UiCommand> {
    let (command_tx, command_rx) = mpsc::channel(UI_COMMAND_QUEUE_CAPACITY);
    runtime.spawn(run_dbus_loop(connection, sender, command_rx));
    command_tx
}

async fn run_dbus_loop(
    connection: Connection,
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<UiCommand>,
) {
    // Buffer UI actions during reconnect to avoid losing user intent.
    let mut offline_commands: VecDeque<UiCommand> = VecDeque::new();
    let mut connect_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut subscribe_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut connect_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));
    let mut subscribe_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));

    loop {
        let proxy = match ControlProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(err) => {
                connect_log.warn_or_debug(&err, "control interface unavailable, retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };
        connect_backoff.reset();
        connect_log.reset();
        info!("connected to unixnotis control interface");

        // Subscribe to signal streams before seeding so match rules are installed
        // early and in-flight events are buffered while the seed request runs.
        let mut added_stream = match proxy.receive_notification_added().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_added");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut updated_stream = match proxy.receive_notification_updated().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_updated");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut closed_stream = match proxy.receive_notification_closed().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_closed");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut state_stream = match proxy.receive_state_changed().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to state_changed");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut panel_stream = match proxy.receive_panel_requested().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to panel_requested");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        subscribe_backoff.reset();
        subscribe_log.reset();

        // Seed after subscriptions to avoid dropping events that arrive during the
        // initial state fetch. The streams buffer messages until polling resumes.
        seed_state_with_retry(&proxy, &sender).await;
        drop_stale_offline_commands(&mut offline_commands);
        flush_offline_commands(&proxy, &sender, &mut offline_commands).await;

        loop {
            tokio::select! {
                command = command_rx.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    if let Err(err) = handle_command(&proxy, &sender, command).await {
                        warn!(?err, "control command failed");
                    }
                }
                signal = added_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_added stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationAdded(
                                args.notification().clone(),
                                *args.show_popup(),
                            ))
                            .await;
                    }
                }
                signal = updated_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_updated stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationUpdated(
                                args.notification().clone(),
                                *args.show_popup(),
                            ))
                            .await;
                    }
                }
                signal = closed_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_closed stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationClosed(
                                *args.id(),
                                *args.reason(),
                            ))
                            .await;
                    }
                }
                signal = state_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("state_changed stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender.send(UiEvent::StateChanged(args.state().clone())).await;
                    }
                }
                signal = panel_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("panel_requested stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender.send(UiEvent::PanelRequested(*args.request())).await;
                    }
                }
            }
        }
        stash_offline_commands(&mut command_rx, &mut offline_commands);
        tokio::time::sleep(subscribe_backoff.next_sleep()).await;
    }
}

async fn seed_state_with_retry(proxy: &ControlProxy<'_>, sender: &async_channel::Sender<UiEvent>) {
    // Seed retries are bounded to keep startup responsive while tolerating transient failures.
    let mut backoff = Backoff::new(SEED_RETRY_BASE_MS, SEED_RETRY_MAX_MS);
    let deadline = Instant::now() + Duration::from_secs(SEED_RETRY_BUDGET_SECS);
    let mut log = RetryLog::new(Duration::from_secs(SEED_RETRY_LOG_INTERVAL_SECS));

    loop {
        match seed_state(proxy, sender).await {
            Ok(()) => return,
            Err(err) => {
                if Instant::now() >= deadline {
                    warn!(
                        state_error = ?err.state_error,
                        active_error = ?err.active_error,
                        history_error = ?err.history_error,
                        "failed to seed center state; giving up until reconnect"
                    );
                    return;
                }
                log.warn_or_debug_seed(&err, "failed to seed center state; retrying");
                tokio::time::sleep(backoff.next_sleep()).await;
            }
        }
    }
}

async fn seed_state(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
) -> Result<(), SeedError> {
    let state = proxy.get_state().await;
    let active = proxy.list_active().await;
    let history = proxy.list_history().await;

    match (state, active, history) {
        (Ok(state), Ok(active), Ok(history)) => {
            let _ = sender
                .send(UiEvent::Seed {
                    state,
                    active,
                    history,
                })
                .await;
            Ok(())
        }
        (state, active, history) => Err(SeedError {
            state_error: state.err().map(|err| err.to_string()),
            active_error: active.err().map(|err| err.to_string()),
            history_error: history.err().map(|err| err.to_string()),
        }),
    }
}

async fn handle_command(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    command: UiCommand,
) -> ZbusResult<()> {
    match command {
        UiCommand::Dismiss(id) => proxy.dismiss(id).await,
        UiCommand::InvokeAction { id, action_key } => proxy.invoke_action(id, &action_key).await,
        UiCommand::ClearAll => {
            proxy.clear_all().await?;
            if let Err(err) = seed_state(proxy, sender).await {
                warn!(
                    state_error = ?err.state_error,
                    active_error = ?err.active_error,
                    history_error = ?err.history_error,
                    "failed to refresh center state after clear"
                );
            }
            Ok(())
        }
        UiCommand::SetDnd(enabled) => proxy.set_dnd(enabled).await,
        UiCommand::ClosePanel => proxy.close_panel().await,
    }
}

const MAX_OFFLINE_COMMANDS: usize = 128;

fn stash_offline_commands(
    command_rx: &mut mpsc::Receiver<UiCommand>,
    offline: &mut VecDeque<UiCommand>,
) {
    let mut drained = 0usize;
    while let Ok(command) = command_rx.try_recv() {
        if offline.len() >= MAX_OFFLINE_COMMANDS {
            offline.pop_front();
            warn!("dropping control command while interface is unavailable");
        }
        offline.push_back(command);
        drained += 1;
    }
    if drained > 0 {
        debug::log(PanelDebugLevel::Info, || {
            format!(
                "buffered {drained} control command(s) while offline (queued={})",
                offline.len()
            )
        });
    }
}

async fn flush_offline_commands(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    offline: &mut VecDeque<UiCommand>,
) {
    if offline.is_empty() {
        return;
    }
    debug::log(PanelDebugLevel::Info, || {
        format!("replaying {} buffered control command(s)", offline.len())
    });
    while let Some(command) = offline.pop_front() {
        if let Err(err) = handle_command(proxy, sender, command).await {
            warn!(?err, "buffered control command failed");
        }
    }
}

fn drop_stale_offline_commands(offline: &mut VecDeque<UiCommand>) {
    // Drop ID-based commands after reconnect to avoid acting on stale IDs.
    let before = offline.len();
    offline.retain(|command| {
        matches!(
            command,
            UiCommand::ClearAll | UiCommand::SetDnd(_) | UiCommand::ClosePanel
        )
    });
    let dropped = before.saturating_sub(offline.len());
    if dropped > 0 {
        debug::log(PanelDebugLevel::Info, || {
            format!("dropped {dropped} stale offline command(s) after reconnect")
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_resets_to_base() {
        let mut backoff = Backoff::new(10, 40);
        let first = backoff.next_sleep();
        assert!(first >= Duration::from_millis(10));

        backoff.next_sleep();
        backoff.next_sleep();
        backoff.reset();

        let reset_sleep = backoff.next_sleep();
        let max = Duration::from_millis(10 + BACKOFF_JITTER_MS);
        assert!(reset_sleep <= max);
    }

    #[test]
    fn backoff_caps_at_max_with_jitter() {
        let mut backoff = Backoff::new(10, 40);
        for _ in 0..10 {
            let sleep = backoff.next_sleep();
            let max = Duration::from_millis(40 + BACKOFF_JITTER_MS);
            assert!(sleep <= max);
        }
    }

    #[test]
    fn jitter_zero_returns_zero() {
        assert_eq!(jitter_duration(0), Duration::from_millis(0));
    }

    #[test]
    fn jitter_duration_is_bounded() {
        // Ensure jitter never exceeds the configured maximum.
        let jitter = jitter_duration(5);
        assert!(jitter <= Duration::from_millis(5));
    }

    #[test]
    fn drop_stale_offline_commands_retains_safe_actions() {
        let mut offline = VecDeque::new();
        offline.push_back(UiCommand::Dismiss(10));
        offline.push_back(UiCommand::InvokeAction {
            id: 11,
            action_key: "open".to_string(),
        });
        offline.push_back(UiCommand::SetDnd(true));
        offline.push_back(UiCommand::ClearAll);
        offline.push_back(UiCommand::ClosePanel);

        drop_stale_offline_commands(&mut offline);

        assert_eq!(offline.len(), 3);
        assert!(offline
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::SetDnd(true))));
        assert!(offline.iter().any(|cmd| matches!(cmd, UiCommand::ClearAll)));
        assert!(offline
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::ClosePanel)));
    }
}
