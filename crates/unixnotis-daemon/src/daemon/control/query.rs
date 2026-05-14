//! Query helpers for ControlServer
//!
//! Keeps read-only control methods grouped outside the main interface file

use unixnotis_core::{ControlState, InhibitorInfo, NotificationView};
use zbus::message::Header;

use super::ControlServer;

impl ControlServer {
    pub(super) async fn query_state(&self, header: &Header<'_>) -> zbus::fdo::Result<ControlState> {
        // State metadata is now treated as privileged control telemetry
        self.authorize_control_call(header, "GetState").await?;
        // Single lock read keeps state snapshot internally consistent
        let store = self.state.store.lock().await;
        // Cheap state snapshot
        Ok(ControlState {
            dnd_enabled: store.dnd_enabled(),
            history_count: store.history_len() as u32,
            inhibited: store.inhibited(),
            inhibitor_count: store.inhibitor_count(),
        })
    }

    pub(super) async fn query_active(
        &self,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        // Guard against untrusted callers before reading any notification content
        self.authorize_control_call(header, "ListActive").await?;
        let store = self.state.store.lock().await;
        // Return active items
        Ok(store.list_active())
    }

    pub(super) async fn query_history(
        &self,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        // History can contain sensitive content, so it uses the same auth gate
        self.authorize_control_call(header, "ListHistory").await?;
        let store = self.state.store.lock().await;
        // Return saved items
        Ok(store.list_history())
    }

    pub(super) async fn query_active_notification(
        &self,
        id: u32,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        // Per-notification fetch keeps full content on an authenticated pull path
        self.authorize_control_call(header, "GetActiveNotification")
            .await?;
        let store = self.state.store.lock().await;
        Ok(store.active_notification_view(id).into_iter().collect())
    }

    pub(super) async fn query_inhibitors(
        &self,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<Vec<InhibitorInfo>> {
        self.authorize_control_call(header, "ListInhibitors")
            .await?;
        let store = self.state.store.lock().await;
        // Returned list is already sorted for deterministic output
        Ok(store.list_inhibitors())
    }
}
