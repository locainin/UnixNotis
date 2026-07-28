use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use tracing::{debug, warn};
use unixnotis_core::{Config, ControlState, Notification, NotificationView, PopupCandidate};

use super::dnd::{DndStateStore, DND_STATE_VERSION};
use super::model::NotificationStore;
use super::notifications::HistoryStore;

impl NotificationStore {
    pub fn new(config: Config) -> Self {
        // Default constructor attempts to bind persistence to XDG state dir
        let dnd_state_store = DndStateStore::new();
        Self::new_with_state_store(config, dnd_state_store)
    }

    pub(crate) fn new_with_state_store(
        config: Config,
        dnd_state_store: Option<DndStateStore>,
    ) -> Self {
        // Config default is used unless a valid persisted value overrides it
        let mut dnd_enabled = config.general.dnd_default;
        let mut dnd_expires_at = None;
        if let Some(store) = dnd_state_store.as_ref() {
            match store.load() {
                Ok(Some(state)) if state.version == DND_STATE_VERSION => {
                    // Versioned state prevents accidental decode of incompatible formats
                    dnd_enabled = state.dnd_enabled;
                    dnd_expires_at = state.dnd_enabled.then_some(state.expires_at).flatten();
                    // A deadline that passed while the daemon was stopped must not revive DND
                    if dnd_expires_at.is_some_and(|expires_at| expires_at <= unix_now_seconds()) {
                        dnd_enabled = false;
                        dnd_expires_at = None;
                        if let Err(err) = store.persist(false, None) {
                            warn!(?err, "failed to clear expired do-not-disturb state");
                        }
                    }
                    debug!(
                        dnd_enabled,
                        ?dnd_expires_at,
                        "loaded persisted do-not-disturb state"
                    );
                }
                Ok(Some(state)) => {
                    // Unknown version is ignored but logged for troubleshooting
                    warn!(
                        version = state.version,
                        "unsupported dnd state version; ignoring persisted value"
                    );
                }
                Ok(None) => {}
                Err(err) => {
                    // Persistence failures must never block daemon startup
                    warn!(?err, "failed to read persisted do-not-disturb state");
                }
            }
        }

        Self {
            // IDs start at 1 to preserve protocol expectations
            next_id: 1,
            // Generation zero stays reserved for payloads not committed to the store
            next_generation: 1,
            dnd_enabled,
            dnd_expires_at,
            dnd_revision: 0,
            config,
            active: IndexMap::new(),
            history: HistoryStore::new(),
            expirations: HashMap::new(),
            dnd_state_store,
            next_inhibitor_id: 1,
            inhibitors: HashMap::new(),
            inhibited: false,
            inhibitor_count: 0,
        }
    }

    pub const fn config(&self) -> &Config {
        &self.config
    }

    pub const fn inhibited(&self) -> bool {
        self.inhibited
    }

    pub const fn inhibitor_count(&self) -> u32 {
        self.inhibitor_count
    }

    pub fn control_state(&self) -> ControlState {
        // One canonical snapshot prevents query and event paths from drifting apart
        ControlState {
            dnd_enabled: self.dnd_enabled(),
            dnd_expires_at: self.dnd_expires_at().unwrap_or(0),
            history_count: self.history_len() as u32,
            inhibited: self.inhibited(),
            inhibitor_count: self.inhibitor_count(),
        }
    }

    pub fn list_active(&self) -> Vec<NotificationView> {
        // Reverse iteration returns newest entries first for panel rendering
        self.active
            .values()
            .rev()
            .map(|notification| notification.to_list_view())
            .collect()
    }

    pub fn list_history(&self) -> Vec<NotificationView> {
        // HistoryStore already returns newest first
        self.history.list_views()
    }

    pub fn list_popup_candidates(&self) -> Vec<NotificationView> {
        // Newest-first ordering matches ListActive while excluding persistent no-popup rules
        self.active
            .values()
            .rev()
            .filter(|notification| !notification.suppress_popup)
            .map(|notification| notification.to_list_view())
            .collect()
    }

    pub fn active_notification_view(&self, id: u32) -> Option<NotificationView> {
        // Active rows use the richer popup-oriented view because add/update signals
        // are consumed by trusted UIs that may need current image payloads
        self.active
            .get(&id)
            .map(|notification| notification.to_view())
    }

    pub fn popup_candidate(&self, id: u32) -> Option<PopupCandidate> {
        // Payload and live gate policy are read from one immutable lock snapshot
        let notification = self.active.get(&id)?;
        Some(PopupCandidate {
            notification: notification.to_view(),
            should_show: self.popup_admission(notification).should_show(),
        })
    }

    pub fn active_inline_reply_target(&self, id: u32) -> Option<Arc<Notification>> {
        let notification = self.active.get(&id)?;
        // Both fields must agree so malformed internal data cannot widen reply access
        let has_reply_action = notification
            .actions
            .iter()
            .any(|action| action.key == "inline-reply");
        (notification.inline_reply.available
            && notification.inline_reply_policy == unixnotis_core::InlineReplyPolicy::Allow
            && has_reply_action)
            .then(|| Arc::clone(notification))
    }

    pub fn active_action_target(&self, id: u32, action_key: &str) -> Option<Arc<Notification>> {
        let notification = self.active.get(&id)?;
        // Weak or conflicting provenance must not gain an application-directed signal
        if !notification.attribution.allows_application_actions() {
            return None;
        }
        // Exact matching prevents a trusted control caller from inventing application actions
        notification
            .actions
            .iter()
            .any(|action| action.key == action_key)
            .then(|| Arc::clone(notification))
    }

    pub fn is_active_notification_generation(&self, id: u32, expected: &Arc<Notification>) -> bool {
        // Arc identity distinguishes a same-ID replacement from the row that was clicked
        self.active
            .get(&id)
            .is_some_and(|active| Arc::ptr_eq(active, expected))
    }

    pub fn history_len(&self) -> usize {
        // Exposed for diagnostics and test assertions
        self.history.len()
    }

    pub fn clear_history(&mut self) {
        // Explicit history wipe used by CLI and control commands
        self.history.clear();
    }
}

fn unix_now_seconds() -> i64 {
    // Chrono handles pre-epoch clocks without panicking
    chrono::Utc::now().timestamp()
}
