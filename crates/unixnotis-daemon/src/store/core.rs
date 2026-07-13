use std::collections::HashMap;

use indexmap::IndexMap;
use tracing::{debug, warn};
use unixnotis_core::{Config, NotificationView};

use super::{DndStateStore, HistoryStore, NotificationStore, DND_STATE_VERSION};

#[cfg(test)]
use std::path::PathBuf;

impl NotificationStore {
    pub fn new(config: Config) -> Self {
        // Default constructor attempts to bind persistence to XDG state dir
        let dnd_state_store = DndStateStore::new();
        Self::new_with_state_store(config, dnd_state_store)
    }

    #[cfg(test)]
    pub(crate) fn new_with_state_dir(config: Config, state_dir: PathBuf) -> Self {
        // Test helper with explicit state path and no env mutations
        let dnd_state_store = Some(DndStateStore::from_state_dir(state_dir));
        Self::new_with_state_store(config, dnd_state_store)
    }

    pub(crate) fn new_with_state_store(
        config: Config,
        dnd_state_store: Option<DndStateStore>,
    ) -> Self {
        // Config default is used unless a valid persisted value overrides it
        let mut dnd_enabled = config.general.dnd_default;
        if let Some(store) = dnd_state_store.as_ref() {
            match store.load() {
                Ok(Some(state)) if state.version == DND_STATE_VERSION => {
                    // Versioned state prevents accidental decode of incompatible formats
                    dnd_enabled = state.dnd_enabled;
                    debug!(dnd_enabled, "loaded persisted do-not-disturb state");
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
            dnd_enabled,
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

    pub fn active_notification_view(&self, id: u32) -> Option<NotificationView> {
        // Active rows use the richer popup-oriented view because add/update signals
        // are consumed by trusted UIs that may need current image payloads
        self.active
            .get(&id)
            .map(|notification| notification.to_view())
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
