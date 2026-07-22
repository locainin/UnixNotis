//! Domain cleanup when a unique D-Bus client disconnects

use crate::daemon::DaemonState;

impl DaemonState {
    pub(in crate::daemon::bus) async fn remove_disconnected_client(&self, owner: &str) {
        // Sender metadata is keyed by unique names and cannot survive owner loss
        self.sender_metadata_cache.remove(owner);

        let inhibitor_change = {
            let mut store = self.store.lock().await;
            if store.remove_inhibitors_by_owner(owner) {
                Some((store.inhibited(), store.inhibitor_count()))
            } else {
                None
            }
        };
        if let Some((active, count)) = inhibitor_change {
            self.publish_inhibitors_changed(active, count, "owner-disconnected")
                .await;
        }
    }
}
