use std::sync::atomic::Ordering;

use unixnotis_core::UiHealth;

use crate::daemon::notifications::{notification_signal_mode_for_sender, NotificationSignalMode};

use super::DaemonState;

impl DaemonState {
    pub(crate) fn set_panel_ready(&self, owner: &str, ready: bool) {
        let mut current_owner = self
            .panel_ready_owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if ready {
            // The latest successful handshake owns the active readiness lease
            *current_owner = Some(owner.to_string());
            self.panel_ready.store(true, Ordering::SeqCst);
        } else if current_owner.as_deref() == Some(owner) {
            // Only the matching center generation can clear its lease
            *current_owner = None;
            self.panel_ready.store(false, Ordering::SeqCst);
        }
    }

    fn clear_panel_ready(&self) {
        let mut current_owner = self
            .panel_ready_owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *current_owner = None;
        self.panel_ready.store(false, Ordering::SeqCst);
    }

    pub(crate) fn set_center_process_running(&self, running: bool) {
        self.center_process_running.store(running, Ordering::SeqCst);
        // Every process generation must complete its own subscription handshake
        self.clear_panel_ready();
    }

    pub(crate) fn set_popups_process_running(&self, running: bool) {
        // Popup health is tracked for supervision and diagnostics
        self.popups_process_running.store(running, Ordering::SeqCst);
        if !running {
            self.clear_popups_ready();
        }
    }

    pub(crate) fn set_popups_ready(&self, owner: &str, ready: bool) {
        let mut current_owner = self
            .popups_ready_owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if ready {
            *current_owner = Some(owner.to_string());
            self.popups_ready.store(true, Ordering::SeqCst);
            self.popups_unready_warning_emitted
                .store(false, Ordering::SeqCst);
        } else if current_owner.as_deref() == Some(owner) {
            *current_owner = None;
            self.popups_ready.store(false, Ordering::SeqCst);
        }
    }

    fn clear_popups_ready(&self) {
        let mut current_owner = self
            .popups_ready_owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *current_owner = None;
        self.popups_ready.store(false, Ordering::SeqCst);
    }

    pub(crate) fn panel_ready(&self) -> bool {
        self.panel_ready.load(Ordering::SeqCst)
    }

    pub(crate) fn popups_ready(&self) -> bool {
        self.popups_ready.load(Ordering::SeqCst)
    }

    pub(crate) fn should_warn_popups_unready(&self) -> bool {
        !self.popups_ready()
            && self
                .popups_unready_warning_emitted
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
    }

    pub(crate) fn ui_health(&self) -> UiHealth {
        UiHealth {
            center_process_running: self.center_process_running.load(Ordering::SeqCst),
            center_ready: self.panel_ready(),
            popups_process_running: self.popups_process_running.load(Ordering::SeqCst),
            popups_ready: self.popups_ready(),
        }
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

    pub(crate) const fn trial_mode(&self) -> bool {
        self.trial_mode
    }

    pub(in crate::daemon) fn control_owner_is_preauthorized(&self, owner: &str) -> bool {
        self.preauthorized_control_owner.as_deref() == Some(owner)
    }
}
