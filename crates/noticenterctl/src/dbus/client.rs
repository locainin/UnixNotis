use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{anyhow, Result};
use unixnotis_core::{ControlProxy, InhibitorInfo, NotificationView, PanelDebugLevel};

const CONTROL_CALL_TIMEOUT: Duration = Duration::from_secs(5);

// A boxed future lets every control command return the same kind of async wrapper
pub(crate) type ControlFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + 'a>>;

// This trait lists every command a control client knows how to ask the daemon to do
pub(crate) trait ControlClient {
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

    // Fetch the notifications that are active right now
    fn list_active(&self) -> ControlFuture<'_, Vec<NotificationView>>;

    // Fetch the notifications that are stored in history
    fn list_history(&self) -> ControlFuture<'_, Vec<NotificationView>>;

    // Turn do-not-disturb on or off directly
    fn set_dnd(&self, enabled: bool) -> ControlFuture<'_, ()>;

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
        // Send the id so the daemon knows exactly which notification to remove
        Box::pin(run_control_call(ControlProxy::dismiss(self, id)))
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

// Runs one daemon call while making sure it cannot hang forever
async fn run_control_call<T>(call: impl Future<Output = zbus::Result<T>>) -> Result<T> {
    // Race the real D-Bus call against the maximum allowed wait time
    match tokio::time::timeout(CONTROL_CALL_TIMEOUT, call).await {
        // The daemon answered in time and the call itself worked
        Ok(Ok(value)) => Ok(value),

        // The daemon answered in time, but reported a D-Bus error
        Ok(Err(err)) => Err(err.into()),

        // The daemon did not answer before the timeout finished
        Err(_) => Err(anyhow!(
            "timed out waiting for unixnotis daemon response after {}s",
            CONTROL_CALL_TIMEOUT.as_secs()
        )),
    }
}
