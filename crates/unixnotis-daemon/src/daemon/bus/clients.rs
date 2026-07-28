//! Domain cleanup when a unique D-Bus client disconnects

use crate::daemon::DaemonState;

impl DaemonState {
    pub(in crate::daemon) async fn remove_disconnected_client(&self, owner: &str) {
        // Sender metadata is keyed by unique names and cannot survive owner loss
        self.sender_metadata_cache.remove(owner);
        // Panel readiness follows the same unique-owner lease as popup readiness
        self.set_panel_ready(owner, false);
        // Only the owner that published the active popup generation can clear it
        self.set_popups_ready(owner, false);

        let inhibitors_removed = {
            let mut store = self.store.lock().await;
            store.remove_inhibitors_by_owner(owner)
        };
        if inhibitors_removed {
            self.publish_inhibitors_changed("owner-disconnected").await;
        }
    }
}
