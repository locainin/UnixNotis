//! Query helpers for `ControlServer`
//!
//! Keeps read-only control methods grouped outside the main interface file

use unixnotis_core::{ControlState, InhibitorInfo, NotificationView, PopupCandidate};
use zbus::message::Header;

use super::ControlServer;

impl ControlServer {
    pub(super) async fn query_state(&self) -> zbus::fdo::Result<ControlState> {
        // Readiness clients receive only aggregate state without notification content
        // Single lock read keeps state snapshot internally consistent
        let store = self.state.store.lock().await;
        Ok(store.control_state())
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

    pub(super) async fn query_popup_candidates(
        &self,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        // Rule-level suppression persists across reconnects and must be applied by the daemon
        self.authorize_control_call(header, "ListPopupCandidates")
            .await?;
        let store = self.state.store.lock().await;
        Ok(store.list_popup_candidates())
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

    pub(super) async fn query_popup_candidate(
        &self,
        id: u32,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<Vec<PopupCandidate>> {
        // Admission and content must describe the same committed generation
        self.authorize_control_call(header, "GetPopupCandidate")
            .await?;
        let store = self.state.store.lock().await;
        Ok(store.popup_candidate(id).into_iter().collect())
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
