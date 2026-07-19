use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use tokio::sync::Mutex;
use unixnotis_core::{Config, ControlState, PopupGateState};
use zbus::Connection;

use crate::dnd_expiration::DndExpirationScheduler;
use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use crate::store::NotificationStore;

use crate::daemon::signal_burst::NotificationBurstState;

/// Shared daemon state guarded behind an async mutex
pub struct DaemonState {
    pub store: Mutex<NotificationStore>,
    /// Immutable sound settings resolved at startup
    pub sound: SoundSettings,
    pub(in crate::daemon::state) connection: Connection,
    // Panel control should only succeed once the center has subscribed
    // This avoids accepting requests that no live listener can receive
    pub(in crate::daemon::state) panel_ready: AtomicBool,
    pub(in crate::daemon::state) popups_running: AtomicBool,
    // Scheduler is installed after state startup so close paths can cancel timers
    pub(in crate::daemon::state) scheduler: OnceLock<ExpirationScheduler>,
    // Warn once if scheduler-backed operations happen before install
    pub(in crate::daemon::state) scheduler_missing_warned: AtomicBool,
    // Timed DND has one coalesced wall-clock deadline
    pub(in crate::daemon::state) dnd_scheduler: OnceLock<DndExpirationScheduler>,
    pub(in crate::daemon::state) dnd_scheduler_missing_warned: AtomicBool,
    // DND persistence and timer replacement must commit in mutation order
    pub(in crate::daemon::state) dnd_write_lock: Mutex<()>,
    // Cache the last control-state snapshot so no-op signals can be skipped
    pub(in crate::daemon) last_emitted_state: StdMutex<Option<ControlState>>,
    // Popup UIs only care about the gate, not panel history counters
    pub(in crate::daemon) last_emitted_popup_gate: StdMutex<Option<PopupGateState>>,
    // Burst tracking lets one noisy sender fall back to snapshot invalidation
    // instead of forcing a storm of full add/update fanout
    pub(in crate::daemon::state) notification_signal_bursts:
        StdMutex<std::collections::HashMap<String, NotificationBurstState>>,
    // Trial mode allows local rebuild loops without forcing daemon restarts for control auth
    pub(in crate::daemon::state) trial_mode: bool,
}

impl DaemonState {
    pub fn new(
        connection: Connection,
        config: Config,
        sound: SoundSettings,
        trial_mode: bool,
    ) -> Arc<Self> {
        let store = NotificationStore::new(config);
        Self::new_with_store(connection, store, sound, trial_mode)
    }

    pub(crate) fn new_with_store(
        connection: Connection,
        store: NotificationStore,
        sound: SoundSettings,
        trial_mode: bool,
    ) -> Arc<Self> {
        // One construction path keeps scheduler, signal cache, and popup state in sync
        Arc::new(Self {
            store: Mutex::new(store),
            sound,
            connection,
            panel_ready: AtomicBool::new(false),
            popups_running: AtomicBool::new(false),
            scheduler: OnceLock::new(),
            scheduler_missing_warned: AtomicBool::new(false),
            dnd_scheduler: OnceLock::new(),
            dnd_scheduler_missing_warned: AtomicBool::new(false),
            dnd_write_lock: Mutex::new(()),
            last_emitted_state: StdMutex::new(None),
            last_emitted_popup_gate: StdMutex::new(None),
            notification_signal_bursts: StdMutex::new(std::collections::HashMap::new()),
            trial_mode,
        })
    }

    pub(crate) const fn connection(&self) -> &Connection {
        &self.connection
    }
}
