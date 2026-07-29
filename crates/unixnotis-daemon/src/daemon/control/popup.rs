//! Popup readiness authorization and owner-generation tracking

use unixnotis_core::{NotificationKey, PopupDeliveryStage};
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

    pub(super) async fn mark_popup_generation_rendered(
        &self,
        key: NotificationKey,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<()> {
        auth::authorize_popup_readiness_call(&self.state, header, "MarkPopupRendered").await?;
        let recorded = self
            .state
            .store
            .lock()
            .await
            .record_popup_delivery_stage(key, PopupDeliveryStage::Rendered);
        if recorded {
            Ok(())
        } else {
            Err(zbus::fdo::Error::InvalidArgs(
                "notification generation is no longer retained".to_string(),
            ))
        }
    }
}
