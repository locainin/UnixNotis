use std::sync::atomic::Ordering;

use unixnotis_core::UiHealth;

use crate::daemon::notifications::{notification_signal_mode_for_sender, NotificationSignalMode};

use super::DaemonState;

impl DaemonState {
    pub(crate) fn set_panel_ready(&self, owner: &str, ready: bool) {
        let mut health = self
            .ui_health
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if ready {
            // The latest successful handshake owns the active readiness lease
            health.panel_ready_owner = Some(owner.to_string());
            health.center_ready = true;
        } else if health.panel_ready_owner.as_deref() == Some(owner) {
            // Only the matching center generation can clear its lease
            health.panel_ready_owner = None;
            health.center_ready = false;
        }
        health.revision = health.revision.saturating_add(1);
    }

    pub(crate) fn set_center_process_running(&self, running: bool) {
        let mut health = self
            .ui_health
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        health.center_process_running = running;
        // Every process generation must complete its own subscription handshake
        health.panel_ready_owner = None;
        health.center_ready = false;
        health.revision = health.revision.saturating_add(1);
    }

    pub(crate) fn set_popups_process_running(&self, running: bool) {
        // Popup health is tracked for supervision and diagnostics
        let mut health = self
            .ui_health
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        health.popups_process_running = running;
        if !running {
            health.popups_ready_owner = None;
            health.popups_ready = false;
        }
        health.revision = health.revision.saturating_add(1);
    }

    pub(crate) fn set_popups_ready(&self, owner: &str, ready: bool) {
        let mut health = self
            .ui_health
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if ready {
            health.popups_ready_owner = Some(owner.to_string());
            health.popups_ready = true;
            self.popups_unready_warning_emitted
                .store(false, Ordering::SeqCst);
        } else if health.popups_ready_owner.as_deref() == Some(owner) {
            health.popups_ready_owner = None;
            health.popups_ready = false;
        }
        health.revision = health.revision.saturating_add(1);
    }

    pub(crate) fn panel_ready(&self) -> bool {
        self.ui_health
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .center_ready
    }

    pub(crate) fn popups_ready(&self) -> bool {
        self.ui_health
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .popups_ready
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "getter is used by child-process tests")
    )]
    pub(crate) fn popups_process_running(&self) -> bool {
        self.ui_health
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .popups_process_running
    }

    pub(crate) fn should_warn_popups_unready(&self) -> bool {
        !self.popups_ready()
            && self
                .popups_unready_warning_emitted
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
    }

    pub(crate) fn ui_health(&self) -> UiHealth {
        // A single read lock prevents mixed fields and revision values
        let health = self
            .ui_health
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        UiHealth {
            center_process_running: health.center_process_running,
            center_ready: health.center_ready,
            popups_process_running: health.popups_process_running,
            popups_ready: health.popups_ready,
            revision: health.revision,
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
