//! D-Bus server implementation and daemon state coordination.
//!
//! The notification and control interfaces are split into submodules to keep
//! responsibilities clear and files smaller.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use tokio::sync::Mutex;
use tracing::warn;
use unixnotis_core::{CloseReason, Config, ControlState, PopupGateState, CONTROL_OBJECT_PATH};
use zbus::{Connection, SignalContext};

use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use crate::store::NotificationStore;

#[path = "daemon/bus_names.rs"]
mod bus_names;
#[path = "daemon/daemon_control.rs"]
mod daemon_control;
#[path = "daemon/daemon_notifications.rs"]
mod daemon_notifications;
#[path = "daemon/signal_burst.rs"]
mod signal_burst;

pub use bus_names::{log_name_reply, request_control_name, request_well_known_name};
pub use daemon_control::{spawn_inhibitor_owner_watch, ControlServer};
pub use daemon_notifications::NotificationServer;
use signal_burst::{
    notification_signal_mode_for_sender, NotificationBurstState, NotificationSignalMode,
};

pub(crate) const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";

/// Shared daemon state guarded behind an async mutex.
pub struct DaemonState {
    pub store: Mutex<NotificationStore>,
    /// Immutable sound settings resolved at startup.
    pub sound: SoundSettings,
    connection: Connection,
    // Panel control should only succeed once the center has subscribed
    // This avoids accepting requests that no live listener can receive
    panel_ready: AtomicBool,
    popups_running: AtomicBool,
    // Scheduler is installed after state startup so close paths can cancel timers
    scheduler: OnceLock<ExpirationScheduler>,
    // Warn once if scheduler-backed operations happen before install
    scheduler_missing_warned: AtomicBool,
    // Cache the last control-state snapshot so no-op signals can be skipped
    last_emitted_state: StdMutex<Option<ControlState>>,
    // Popup UIs only care about the gate, not panel history counters
    last_emitted_popup_gate: StdMutex<Option<PopupGateState>>,
    // Burst tracking lets one noisy sender fall back to snapshot invalidation
    // instead of forcing a storm of full add/update fanout
    notification_signal_bursts: StdMutex<std::collections::HashMap<String, NotificationBurstState>>,
}

impl DaemonState {
    pub fn new(connection: Connection, config: Config, sound: SoundSettings) -> Arc<Self> {
        let store = NotificationStore::new(config);
        Arc::new(Self {
            store: Mutex::new(store),
            sound,
            connection,
            panel_ready: AtomicBool::new(false),
            popups_running: AtomicBool::new(false),
            scheduler: OnceLock::new(),
            scheduler_missing_warned: AtomicBool::new(false),
            last_emitted_state: StdMutex::new(None),
            last_emitted_popup_gate: StdMutex::new(None),
            notification_signal_bursts: StdMutex::new(std::collections::HashMap::new()),
        })
    }

    pub fn set_scheduler(&self, scheduler: ExpirationScheduler) {
        // Scheduler is wired once during daemon startup
        if self.scheduler.set(scheduler).is_err() {
            warn!("expiration scheduler was already installed; ignoring duplicate initialization");
            return;
        }
        self.scheduler_missing_warned.store(false, Ordering::SeqCst);
    }

    fn scheduler(&self) -> Option<ExpirationScheduler> {
        // Cloning the sender handle is cheap and keeps await points simple
        let scheduler = self.scheduler.get().cloned();
        if scheduler.is_none() && !self.scheduler_missing_warned.swap(true, Ordering::SeqCst) {
            warn!("expiration scheduler is unavailable during live daemon operation");
        }
        scheduler
    }

    async fn cancel_expiration(&self, id: u32) {
        // Missing scheduler means startup is still incomplete, so skip quietly
        let Some(scheduler) = self.scheduler() else {
            return;
        };
        scheduler.schedule(id, None).await;
    }

    pub async fn cancel_expirations(&self, ids: &[u32]) {
        // Cancel timers for every removed active id so stale wakeups do not build up
        // Per-id cancel keeps the existing lazy heap design simple and predictable
        let Some(scheduler) = self.scheduler() else {
            return;
        };
        for id in ids {
            scheduler.schedule(*id, None).await;
        }
    }

    pub async fn close_notification(&self, id: u32, reason: CloseReason) -> zbus::Result<()> {
        let removed = {
            let mut store = self.store.lock().await;
            store.close(id, reason)
        };
        if removed.is_none() {
            return Ok(());
        }
        // Timer cancel happens before signal fanout so stale wakeups stop right away
        self.cancel_expiration(id).await;

        if let Err(err) = self.emit_close_fanout(id, reason).await {
            warn!(
                ?err,
                id,
                reason = reason as u32,
                "notification close committed but one or more D-Bus signals failed"
            );
        }
        Ok(())
    }

    pub async fn dismiss_from_panel(&self, id: u32) -> zbus::Result<()> {
        let outcome = {
            let mut store = self.store.lock().await;
            store.dismiss_from_panel(id)
        };

        if !outcome.removed_any() {
            return Ok(());
        }

        if outcome.removed_active {
            // Panel dismiss removes the active entry, so its timer must go too
            self.cancel_expiration(id).await;
        }
        if let Err(err) = self.emit_dismiss_fanout(id, outcome.removed_active).await {
            warn!(
                ?err,
                id, "panel dismiss committed but one or more D-Bus signals failed"
            );
        }
        Ok(())
    }

    async fn emit_close_fanout(&self, id: u32, reason: CloseReason) -> zbus::Result<()> {
        let notif_ctx = SignalContext::new(&self.connection, NOTIFICATIONS_OBJECT_PATH)?;
        NotificationServer::notification_closed(&notif_ctx, id, reason as u32).await?;

        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::notification_closed(&control_ctx, id, reason).await?;
        self.emit_state_changed().await
    }

    async fn emit_dismiss_fanout(&self, id: u32, removed_active: bool) -> zbus::Result<()> {
        if removed_active {
            let notif_ctx = SignalContext::new(&self.connection, NOTIFICATIONS_OBJECT_PATH)?;
            NotificationServer::notification_closed(
                &notif_ctx,
                id,
                CloseReason::DismissedByUser as u32,
            )
            .await?;
        }

        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::notification_closed(&control_ctx, id, CloseReason::DismissedByUser).await?;
        self.emit_state_changed().await
    }

    async fn emit_state_changed(&self) -> zbus::Result<()> {
        let state = {
            let store = self.store.lock().await;
            let history_count = store.history_len() as u32;
            // Panel consumers still need history and inhibitor counters in one snapshot
            ControlState {
                dnd_enabled: store.dnd_enabled(),
                history_count,
                inhibited: store.inhibited(),
                inhibitor_count: store.inhibitor_count(),
            }
        };
        // Popup policy only depends on the gate, so history churn should not wake it up
        let popup_gate = PopupGateState {
            dnd_enabled: state.dnd_enabled,
            inhibited: state.inhibited,
        };
        // Duplicate broadcasts add D-Bus churn without changing UI behavior
        let should_emit_state = should_emit_cached(&self.last_emitted_state, &state);
        let should_emit_popup_gate = should_emit_cached(&self.last_emitted_popup_gate, &popup_gate);
        if !should_emit_state && !should_emit_popup_gate {
            return Ok(());
        }
        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        if should_emit_state {
            ControlServer::state_changed(&control_ctx, state).await?;
        }
        if should_emit_popup_gate {
            ControlServer::popup_gate_changed(&control_ctx, popup_gate).await?;
        }
        Ok(())
    }

    pub async fn emit_snapshot_invalidated(&self) -> zbus::Result<()> {
        // This signal tells clients their local materialized view may be stale
        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::snapshot_invalidated(&control_ctx).await
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    pub(crate) fn set_panel_ready(&self, ready: bool) {
        // SeqCst keeps state changes easy to follow during crash recovery
        self.panel_ready.store(ready, Ordering::SeqCst);
    }

    pub(crate) fn set_popups_running(&self, running: bool) {
        // Popup health is tracked for supervision and diagnostics
        self.popups_running.store(running, Ordering::SeqCst);
    }

    pub(crate) fn panel_ready(&self) -> bool {
        self.panel_ready.load(Ordering::SeqCst)
    }

    pub(crate) fn notification_signal_mode(
        &self,
        sender_name: Option<&str>,
    ) -> NotificationSignalMode {
        notification_signal_mode_for_sender(
            &self.notification_signal_bursts,
            sender_name.unwrap_or("<unknown>"),
        )
    }
}

pub(crate) fn to_fdo_error(err: zbus::Error) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(err.to_string())
}

fn should_emit_cached<T: Clone + PartialEq>(cache: &StdMutex<Option<T>>, value: &T) -> bool {
    // Sync mutex is enough here because this cache is tiny and never held across await points
    let mut last_value = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if last_value
        .as_ref()
        .is_some_and(|previous| previous == value)
    {
        // Identical state would only burn CPU in zbus and the listeners
        return false;
    }
    // Clone once on change so later comparisons stay allocation-free for equal values
    *last_value = Some(value.clone());
    true
}
