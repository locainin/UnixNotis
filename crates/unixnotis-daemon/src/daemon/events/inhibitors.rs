//! Inhibitor event fanout after store updates and client disconnects

use tracing::warn;

use crate::daemon::{ControlServer, DaemonState};

use super::publisher::DaemonEventPublisher;

impl DaemonState {
    pub(in crate::daemon) async fn publish_inhibitors_changed(&self, action: &'static str) {
        let _publication = self.events.ordered_publication().await;
        let state = {
            // Read the count only after ordering so stale captured values cannot fan out later
            let store = self.store.lock().await;
            store.control_state()
        };
        let active = state.inhibited;
        let count = state.inhibitor_count;
        if let Err(error) = self.events.inhibitors_changed(active, count).await {
            warn!(
                ?error,
                inhibitor_count = count,
                action,
                "inhibitor mutation committed but inhibitor fanout failed"
            );
        }
        if let Err(error) = self.events.state_changed(state).await {
            warn!(
                ?error,
                action, "inhibitor mutation committed but state fanout failed"
            );
        }
    }
}

impl DaemonEventPublisher {
    async fn inhibitors_changed(&self, active: bool, count: u32) -> zbus::Result<()> {
        let context = self.control_context()?;
        ControlServer::inhibitors_changed(&context, active, count).await
    }
}
