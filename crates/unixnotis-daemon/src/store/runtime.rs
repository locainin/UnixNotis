use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use tracing::{debug, warn};
use unixnotis_core::{
    ApplicationActionPolicy, Config, ControlState, Notification, NotificationDiagnosticsView,
    NotificationKey, NotificationView, PopupAdmissionView, PopupCandidate, PopupDecisionRecord,
    PopupDeliveryStage, UiHealth,
};

use super::dnd::{DndStateStore, DND_STATE_VERSION};
use super::model::{DeliveryStageUpdate, NotificationStore};
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
            popup_decisions: HashMap::new(),
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
            .map(|notification| self.list_view_with_popup_decision(notification))
            .collect()
    }

    pub fn list_history(&self) -> Vec<NotificationView> {
        // HistoryStore already returns newest first
        self.history.list_views(&self.popup_decisions)
    }

    pub fn list_popup_candidates(&self) -> Vec<NotificationView> {
        // Newest-first ordering matches ListActive while excluding persistent no-popup rules
        self.active
            .values()
            .rev()
            .filter(|notification| {
                !notification.suppress_popup
                    && self
                        .popup_decisions
                        .get(&notification.key())
                        .is_some_and(|decision| {
                            matches!(
                                decision.admission_at_commit,
                                PopupAdmissionView::Show | PopupAdmissionView::RendererUnavailable
                            )
                        })
            })
            .map(|notification| self.list_view_with_popup_decision(notification))
            .collect()
    }

    pub fn active_notification_view(&self, id: u32) -> Option<NotificationView> {
        // Active rows use the richer popup-oriented view because add/update signals
        // are consumed by trusted UIs that may need current image payloads
        self.active
            .get(&id)
            .map(|notification| self.view_with_popup_decision(notification))
    }

    pub fn popup_candidate(&mut self, id: u32) -> Option<PopupCandidate> {
        // Payload and its arrival-time policy are read from one store-lock snapshot
        let notification = self.active.get(&id)?;
        let key = notification.key();
        let admission = self.popup_decisions.get(&key)?.admission_at_commit;
        let view = self.view_with_popup_decision(notification);
        if admission.should_show() {
            self.record_popup_delivery_stage(key, PopupDeliveryStage::RendererFetched);
        }
        Some(PopupCandidate {
            notification: view,
            admission,
        })
    }

    pub fn notification_diagnostics(
        &self,
        id: u32,
        _ui_health: &UiHealth,
    ) -> Option<NotificationDiagnosticsView> {
        let notification = self.active.get(&id).or_else(|| self.history.get(&id))?;
        let decision = self.popup_decisions.get(&notification.key())?;

        Some(NotificationDiagnosticsView {
            id,
            generation: notification.generation,
            stored: true,
            attribution: notification.attribution_diagnostics.clone(),
            identity_assurance: notification.attribution.assurance,
            interaction_policies: notification.attribution.interactions,
            popup_admission: decision.admission_at_commit,
            renderer_process_running: decision.renderer_process_running_at_commit,
            renderer_ready: decision.renderer_ready_at_commit,
            configured_max_visible: decision.max_visible_at_commit,
            decided_at_unix_ms: decision.decided_at_unix_ms,
            delivery_stage: decision.delivery_stage,
        })
    }

    pub(crate) fn record_popup_commit_environment(
        &mut self,
        key: NotificationKey,
        admission: super::PopupAdmission,
        ui_health: &UiHealth,
    ) {
        let max_visible = u32::try_from(self.config.popups.max_visible).unwrap_or(u32::MAX);
        let effective_admission = if !admission.should_show() {
            admission.to_view()
        } else if max_visible == 0 {
            PopupAdmissionView::RendererDisabled
        } else if ui_health.popups_process_running && ui_health.popups_ready {
            PopupAdmissionView::Show
        } else {
            PopupAdmissionView::RendererUnavailable
        };
        let delivery_stage = if effective_admission.should_show() {
            PopupDeliveryStage::Admitted
        } else {
            PopupDeliveryStage::Suppressed
        };
        self.popup_decisions.insert(
            key,
            PopupDecisionRecord {
                admission_at_commit: effective_admission,
                renderer_process_running_at_commit: ui_health.popups_process_running,
                renderer_ready_at_commit: ui_health.popups_ready,
                max_visible_at_commit: max_visible,
                decided_at_unix_ms: chrono::Utc::now().timestamp_millis(),
                delivery_stage,
            },
        );
    }

    pub fn record_popup_delivery_stage(
        &mut self,
        key: NotificationKey,
        next: PopupDeliveryStage,
    ) -> DeliveryStageUpdate {
        let Some(decision) = self.popup_decisions.get_mut(&key) else {
            return DeliveryStageUpdate::MissingGeneration;
        };
        // Duplicate or delayed acknowledgements cannot rewrite retained history
        if next.rank() <= decision.delivery_stage.rank() {
            return DeliveryStageUpdate::AlreadyAtOrBeyond;
        }
        decision.delivery_stage = next;
        DeliveryStageUpdate::Advanced
    }

    pub(super) fn prune_popup_decisions(&mut self) {
        self.popup_decisions.retain(|key, _decision| {
            self.active
                .get(&key.id)
                .is_some_and(|notification| notification.generation == key.generation)
                || self.history.contains_generation(*key)
        });
    }

    fn view_with_popup_decision(&self, notification: &Notification) -> NotificationView {
        let mut view = notification.to_view();
        if let Some(decision) = self.popup_decisions.get(&notification.key()) {
            view.popup_decision.clone_from(decision);
        }
        view
    }

    fn list_view_with_popup_decision(&self, notification: &Notification) -> NotificationView {
        let mut view = notification.to_list_view();
        if let Some(decision) = self.popup_decisions.get(&notification.key()) {
            view.popup_decision.clone_from(decision);
        }
        view
    }

    pub fn active_inline_reply_target(
        &self,
        id: u32,
        generation: u64,
    ) -> Option<Arc<Notification>> {
        let notification = self.active.get(&id)?;
        // Both fields must agree so malformed internal data cannot widen reply access
        let has_reply_action = notification
            .actions
            .iter()
            .any(|action| action.key == "inline-reply");
        (notification.inline_reply.available
            && notification.generation == generation
            && notification.attribution.interactions.inline_reply
                == unixnotis_core::InlineReplyPolicy::Allow
            && notification.inline_reply_policy == unixnotis_core::InlineReplyPolicy::Allow
            && has_reply_action)
            .then(|| Arc::clone(notification))
    }

    pub fn active_action_target_generation(
        &self,
        key: unixnotis_core::NotificationKey,
        action_key: &str,
        confirmed: bool,
    ) -> Option<Arc<Notification>> {
        let notification = self.active.get(&key.id)?;
        if notification.generation != key.generation {
            return None;
        }
        // "inline-reply" is a fake action key used only by the reply text method
        // Block it here even though action_policy already rejects it — that way a caller
        // that skips the policy check still cannot reach the reply action
        if action_key == "inline-reply" {
            return None;
        }
        // Confirmation is meaningful only for actions the resolver explicitly marked confirmable
        let policy = notification.attribution.action_policy(action_key);
        let authorized = match policy {
            ApplicationActionPolicy::Allow => true,
            ApplicationActionPolicy::Confirm => confirmed,
            ApplicationActionPolicy::Deny => false,
        };
        if !authorized {
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
        self.prune_popup_decisions();
    }
}

fn unix_now_seconds() -> i64 {
    // Chrono handles pre-epoch clocks without panicking
    chrono::Utc::now().timestamp()
}
