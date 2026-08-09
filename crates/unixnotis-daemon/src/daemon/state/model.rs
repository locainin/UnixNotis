use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex, OnceLock, RwLock as StdRwLock};

use arc_swap::ArcSwap;
use tokio::sync::Mutex;
use unixnotis_core::Config;
use zbus::Connection;

use crate::daemon::auth::{
    build_trusted_control_snapshots_for_current_executable, TrustedExecutableSnapshot,
};
use crate::dnd_expiration::DndExpirationScheduler;
use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use crate::store::NotificationStore;

use crate::daemon::events::DaemonEventPublisher;
use crate::daemon::notifications::identity::{DesktopIdentityIndex, DesktopIndexRefreshHandle};
use crate::daemon::notifications::NotificationBurstState;
use crate::daemon::notifications::SenderMetadataCache;
use crate::daemon::state::InteractionGates;

#[derive(Clone, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the control protocol exposes four independent readiness flags"
)]
pub(in crate::daemon::state) struct UiHealthState {
    pub(in crate::daemon::state) center_process_running: bool,
    pub(in crate::daemon::state) center_ready: bool,
    pub(in crate::daemon::state) panel_ready_owner: Option<String>,
    pub(in crate::daemon::state) popups_process_running: bool,
    pub(in crate::daemon::state) popups_ready: bool,
    pub(in crate::daemon::state) popups_ready_owner: Option<String>,
    pub(in crate::daemon::state) revision: u64,
}

/// Shared daemon state guarded behind an async mutex
pub struct DaemonState {
    pub store: Mutex<NotificationStore>,
    // Action, reply, and replacement commits for one numeric ID share this bounded gate
    pub(in crate::daemon) interaction_gates: InteractionGates,
    // This map is built before the control object is exported and never rebuilt from callers
    pub(in crate::daemon) trusted_executables: Arc<HashMap<String, TrustedExecutableSnapshot>>,
    /// Immutable sound settings resolved at startup
    pub sound: SoundSettings,
    pub(in crate::daemon::state) connection: Connection,
    // Panel control should only succeed once the center has subscribed
    // This avoids accepting requests that no live listener can receive
    // One lock keeps process, readiness, owner, and revision values coherent
    pub(in crate::daemon::state) ui_health: StdRwLock<UiHealthState>,
    pub(in crate::daemon::state) popups_unready_warning_emitted: AtomicBool,
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
    // Readers load one immutable snapshot while filesystem refresh swaps the complete index
    pub(crate) desktop_identity_index: Arc<ArcSwap<DesktopIdentityIndex>>,
    // The refresh worker owns watcher replacement and atomic index publication
    pub(in crate::daemon::state) desktop_index_refresh: OnceLock<DesktopIndexRefreshHandle>,
    // Trial mode allows local rebuild loops without forcing daemon restarts for control auth
    pub(in crate::daemon::state) trial_mode: bool,
    // Normal startup supplies None; private-bus protocol tests can inject one unique owner
    pub(in crate::daemon::state) preauthorized_control_owner: Option<String>,
}

impl DaemonState {
    pub fn new(
        connection: Connection,
        config: Config,
        sound: SoundSettings,
        trial_mode: bool,
        desktop_identity_index: Arc<ArcSwap<DesktopIdentityIndex>>,
        preauthorized_control_owner: Option<String>,
    ) -> Arc<Self> {
        let store = NotificationStore::new(config);
        Self::new_with_store(
            connection,
            store,
            sound,
            trial_mode,
            desktop_identity_index,
            preauthorized_control_owner,
        )
    }

    pub(crate) fn new_with_store(
        connection: Connection,
        store: NotificationStore,
        sound: SoundSettings,
        trial_mode: bool,
        desktop_identity_index: Arc<ArcSwap<DesktopIdentityIndex>>,
        preauthorized_control_owner: Option<String>,
    ) -> Arc<Self> {
        // One construction path keeps scheduler, signal cache, and popup state in sync
        let trusted_executables =
            Arc::new(build_trusted_control_snapshots_for_current_executable());
        Arc::new(Self {
            store: Mutex::new(store),
            interaction_gates: InteractionGates::new(),
            trusted_executables,
            sound,
            connection: connection.clone(),
            ui_health: StdRwLock::new(UiHealthState::default()),
            popups_unready_warning_emitted: AtomicBool::new(false),
            scheduler: OnceLock::new(),
            scheduler_missing_warned: AtomicBool::new(false),
            dnd_scheduler: OnceLock::new(),
            dnd_scheduler_missing_warned: AtomicBool::new(false),
            dnd_write_lock: Mutex::new(()),
            events: DaemonEventPublisher::new(connection),
            notification_signal_bursts: StdMutex::new(std::collections::HashMap::new()),
            sender_metadata_cache: SenderMetadataCache::new(),
            desktop_identity_index,
            desktop_index_refresh: OnceLock::new(),
            trial_mode,
            preauthorized_control_owner,
        })
    }

    pub(crate) const fn connection(&self) -> &Connection {
        &self.connection
    }

    pub(in crate::daemon) fn trusted_executables(
        &self,
    ) -> &HashMap<String, TrustedExecutableSnapshot> {
        &self.trusted_executables
    }

    pub(crate) fn set_desktop_index_refresh(&self, handle: DesktopIndexRefreshHandle) {
        let _ = self.desktop_index_refresh.set(handle);
    }

    pub(crate) fn request_desktop_index_refresh(&self) -> bool {
        self.desktop_index_refresh
            .get()
            .is_some_and(DesktopIndexRefreshHandle::request_manual)
    }
}
