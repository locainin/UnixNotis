use std::future::Future;
use std::pin::Pin;

use anyhow::{anyhow, Result};
use unixnotis_core::{
    ControlProxy, InhibitorInfo, NotificationDiagnosticsView, NotificationView, PanelDebugLevel,
};

use super::timeout::run_control_call;

// A boxed future lets every control command return the same kind of async wrapper
pub type ControlFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + 'a>>;

// This trait lists every command a control client knows how to ask the daemon to do
pub trait ControlClient {
    // Ask the panel to switch between open and closed
    fn toggle_panel(&self) -> ControlFuture<'_, ()>;

    // Ask the panel to open normally
    fn open_panel(&self) -> ControlFuture<'_, ()>;

    // Ask the panel to open with extra debug information
    fn open_panel_debug(&self, level: PanelDebugLevel) -> ControlFuture<'_, ()>;

    // Ask the panel to close
    fn close_panel(&self) -> ControlFuture<'_, ()>;

    // Remove every notification from both active and history areas
    fn clear_all(&self) -> ControlFuture<'_, ()>;

    // Remove only the notifications that are currently active
    fn clear_active(&self) -> ControlFuture<'_, ()>;

    // Remove only the notifications saved in history
    fn clear_history(&self) -> ControlFuture<'_, ()>;

    // Remove one notification by its id
    fn dismiss(&self, id: u32) -> ControlFuture<'_, ()>;

    // Fetch structured attribution and popup state for one active notification
    fn notification_diagnostics(
        &self,
        id: u32,
    ) -> ControlFuture<'_, Vec<NotificationDiagnosticsView>>;

    // Fetch the notifications that are active right now
    fn list_active(&self) -> ControlFuture<'_, Vec<NotificationView>>;

    // Fetch the notifications that are stored in history
    fn list_history(&self) -> ControlFuture<'_, Vec<NotificationView>>;

    // Turn do-not-disturb on or off directly
    fn set_dnd(&self, enabled: bool) -> ControlFuture<'_, ()>;

    // Enable do-not-disturb until one absolute deadline
    fn set_dnd_until(&self, expires_at: i64) -> ControlFuture<'_, ()>;

    // Flip do-not-disturb to the opposite of what it is now
    fn toggle_dnd(&self) -> ControlFuture<'_, ()>;

    // Add a temporary blocker that can stop notifications for a reason and scope
    fn inhibit<'a>(&'a self, reason: &'a str, scope: u32) -> ControlFuture<'a, u64>;

    // Remove an existing blocker by its id
    fn uninhibit(&self, id: u64) -> ControlFuture<'_, ()>;

    // Fetch the list of blockers that are currently active
    fn list_inhibitors(&self) -> ControlFuture<'_, Vec<InhibitorInfo>>;
}

// This makes the real D-Bus proxy usable through the simpler ControlClient trait
impl ControlClient for ControlProxy<'_> {
    fn toggle_panel(&self) -> ControlFuture<'_, ()> {
        // Start the proxy call and wrap it in the shared timeout and error handling helper
        Box::pin(run_control_call(ControlProxy::toggle_panel(self)))
    }

    fn open_panel(&self) -> ControlFuture<'_, ()> {
        // Send the open request through the proxy using the common call wrapper
        Box::pin(run_control_call(ControlProxy::open_panel(self)))
    }

    fn open_panel_debug(&self, level: PanelDebugLevel) -> ControlFuture<'_, ()> {
        // Pass the debug level along so the daemon knows how much extra info to show
        Box::pin(run_control_call(ControlProxy::open_panel_debug(
            self, level,
        )))
    }

    fn close_panel(&self) -> ControlFuture<'_, ()> {
        // Send the close request and let the helper deal with timeout and errors
        Box::pin(run_control_call(ControlProxy::close_panel(self)))
    }

    fn clear_all(&self) -> ControlFuture<'_, ()> {
        // Ask the daemon to clear everything it is holding
        Box::pin(run_control_call(ControlProxy::clear_all(self)))
    }

    fn clear_active(&self) -> ControlFuture<'_, ()> {
        // Ask the daemon to clear only the notifications that are still live
        Box::pin(run_control_call(ControlProxy::clear_active(self)))
    }

    fn clear_history(&self) -> ControlFuture<'_, ()> {
        // Ask the daemon to clear only the older saved notifications
        Box::pin(run_control_call(ControlProxy::clear_history(self)))
    }

    fn dismiss(&self, id: u32) -> ControlFuture<'_, ()> {
        Box::pin(async move {
            // Resolve one exact active generation before issuing the mutating call
            let mut candidates =
                run_control_call(ControlProxy::get_active_notification(self, id)).await?;
            let notification = candidates
                .pop()
                .ok_or_else(|| anyhow!("notification {id} is not active"))?;
            run_control_call(ControlProxy::dismiss_generation(
                self,
                notification.id,
                notification.generation,
            ))
            .await
        })
    }

    fn notification_diagnostics(
        &self,
        id: u32,
    ) -> ControlFuture<'_, Vec<NotificationDiagnosticsView>> {
        Box::pin(run_control_call(
            ControlProxy::get_notification_diagnostics(self, id),
        ))
    }

    fn list_active(&self) -> ControlFuture<'_, Vec<NotificationView>> {
        // Ask for the current active notifications and return them as view-friendly data
        Box::pin(run_control_call(ControlProxy::list_active(self)))
    }

    fn list_history(&self) -> ControlFuture<'_, Vec<NotificationView>> {
        // Ask for the saved notification history and return it as view-friendly data
        Box::pin(run_control_call(ControlProxy::list_history(self)))
    }

    fn set_dnd(&self, enabled: bool) -> ControlFuture<'_, ()> {
        // Send the exact do-not-disturb value the caller wants
        Box::pin(run_control_call(ControlProxy::set_dnd(self, enabled)))
    }

    fn set_dnd_until(&self, expires_at: i64) -> ControlFuture<'_, ()> {
        // Absolute timestamps keep CLI and panel deadlines consistent across daemon restarts
        Box::pin(run_control_call(ControlProxy::set_dnd_until(
            self, expires_at,
        )))
    }

    fn toggle_dnd(&self) -> ControlFuture<'_, ()> {
        // Ask the daemon to flip do-not-disturb without the caller needing to know its current value
        Box::pin(run_control_call(ControlProxy::toggle_dnd(self)))
    }

    fn inhibit<'a>(&'a self, reason: &'a str, scope: u32) -> ControlFuture<'a, u64> {
        // Send the reason and scope, then get back an id that can be used to undo it later
        Box::pin(run_control_call(ControlProxy::inhibit(self, reason, scope)))
    }

    fn uninhibit(&self, id: u64) -> ControlFuture<'_, ()> {
        // Tell the daemon which blocker id should be removed
        Box::pin(run_control_call(ControlProxy::uninhibit(self, id)))
    }

    fn list_inhibitors(&self) -> ControlFuture<'_, Vec<InhibitorInfo>> {
        // Ask the daemon for all current blockers and their details
        Box::pin(run_control_call(ControlProxy::list_inhibitors(self)))
    }
}
