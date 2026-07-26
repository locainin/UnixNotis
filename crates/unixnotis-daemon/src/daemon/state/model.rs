use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use tokio::sync::Mutex;
use unixnotis_core::Config;
use zbus::Connection;

use crate::dnd_expiration::DndExpirationScheduler;
use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use crate::store::NotificationStore;

use crate::daemon::events::DaemonEventPublisher;
use crate::daemon::notifications::identity::DesktopIdentityIndex;
use crate::daemon::notifications::NotificationBurstState;
use crate::daemon::notifications::SenderMetadataCache;

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
    // Connection-facing signal policy stays outside mutable domain state
    pub(in crate::daemon) events: DaemonEventPublisher,
    // Burst tracking lets one noisy sender fall back to snapshot invalidation
    // instead of forcing a storm of full add/update fanout
    pub(in crate::daemon::state) notification_signal_bursts:
        StdMutex<std::collections::HashMap<String, NotificationBurstState>>,
    // Unique sender identities avoid repeated bus and procfs lookups during bursts
    pub(in crate::daemon) sender_metadata_cache: SenderMetadataCache,
    // Desktop records are indexed once so notification bursts never rescan application files
    pub(in crate::daemon) desktop_identity_index: Arc<DesktopIdentityIndex>,
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
            connection: connection.clone(),
            panel_ready: AtomicBool::new(false),
            popups_running: AtomicBool::new(false),
            scheduler: OnceLock::new(),
            scheduler_missing_warned: AtomicBool::new(false),
            dnd_scheduler: OnceLock::new(),
            dnd_scheduler_missing_warned: AtomicBool::new(false),
            dnd_write_lock: Mutex::new(()),
            events: DaemonEventPublisher::new(connection),
            notification_signal_bursts: StdMutex::new(std::collections::HashMap::new()),
            sender_metadata_cache: SenderMetadataCache::new(),
            desktop_identity_index: DesktopIdentityIndex::shared(),
            trial_mode,
        })
    }

    pub(crate) const fn connection(&self) -> &Connection {
        &self.connection
    }
}
