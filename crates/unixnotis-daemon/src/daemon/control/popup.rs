//! Popup readiness authorization and owner-generation tracking

use zbus::message::Header;

use super::ControlServer;
use crate::daemon::auth;

impl ControlServer {
    pub(super) async fn set_popups_ready_state(
        &self,
        header: &Header<'_>,
        method: &'static str,
        ready: bool,
    ) -> zbus::fdo::Result<()> {
        // Executable verification runs before trusting the broker-supplied unique owner
        auth::authorize_popup_readiness_call(&self.state, header, method).await?;
        let owner = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::AccessDenied("missing sender".to_string()))?;
        self.state.set_popups_ready(owner.as_str(), ready);
        Ok(())
    }
}
