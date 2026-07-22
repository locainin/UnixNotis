//! Inhibitor event fanout after store updates and client disconnects

use tracing::warn;

use crate::daemon::{ControlServer, DaemonState};

use super::publisher::DaemonEventPublisher;

impl DaemonState {
    pub(in crate::daemon) async fn publish_inhibitors_changed(
        &self,
        active: bool,
        count: u32,
        action: &'static str,
    ) {
        if let Err(error) = self.events.inhibitors_changed(active, count).await {
            warn!(
                ?error,
                inhibitor_count = count,
                action,
                "inhibitor mutation committed but inhibitor fanout failed"
            );
        }
        if let Err(error) = self.publish_state_changed().await {
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
