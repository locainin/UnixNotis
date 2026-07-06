use std::sync::atomic::Ordering;

use crate::daemon::signal_burst::{notification_signal_mode_for_sender, NotificationSignalMode};

use super::DaemonState;

impl DaemonState {
    pub(crate) fn set_panel_ready(&self, ready: bool) {
        // SeqCst keeps state changes easy to follow during crash recovery
        self.panel_ready.store(ready, Ordering::SeqCst);
    }

    pub(crate) fn set_popups_running(&self, running: bool) {
        // Popup health is tracked for supervision and diagnostics
        self.popups_running.store(running, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn popups_running(&self) -> bool {
        // Read path is used by supervision tests and diagnostics without mutating daemon state
        self.popups_running.load(Ordering::SeqCst)
    }

    pub(crate) fn panel_ready(&self) -> bool {
        self.panel_ready.load(Ordering::SeqCst)
    }

    pub(crate) fn notification_signal_mode(
        &self,
        sender_name: Option<&str>,
    ) -> NotificationSignalMode {
        notification_signal_mode_for_sender(
            &self.notification_signal_bursts,
            sender_name.unwrap_or("<unknown>"),
        )
    }

    pub(crate) fn trial_mode(&self) -> bool {
        self.trial_mode
    }
}
