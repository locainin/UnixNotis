//! Panel request/readiness helpers for ControlServer
//!
//! Keeps panel lifecycle and request fanout logic isolated from other control methods

use unixnotis_core::{PanelRequest, CONTROL_OBJECT_PATH};
use zbus::message::Header;
use zbus::SignalContext;

use crate::daemon::to_fdo_error;

use super::ControlServer;

impl ControlServer {
    pub(super) async fn request_panel_command(
        &self,
        header: &Header<'_>,
        method: &'static str,
        request: PanelRequest,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(header, method).await?;
        self.ensure_panel_available()?;
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Panel requests are signaled so center keeps authority over local UI policy
        ControlServer::panel_requested(&ctx, request)
            .await
            .map_err(to_fdo_error)
    }

    pub(super) async fn set_panel_ready_state(
        &self,
        header: &Header<'_>,
        method: &'static str,
        ready: bool,
    ) -> zbus::fdo::Result<()> {
        self.authorize_panel_readiness_call(header, method).await?;
        // Center reports ready only after it is subscribed to panel_requested
        self.state.set_panel_ready(ready);
        Ok(())
    }
}
