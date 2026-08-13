//! Notification-daemon lifecycle boundaries used by installer actions

mod name_reservation;
mod process_handle;
mod quiescence;
mod stop;

pub use name_reservation::DaemonActivationReservation;
pub use quiescence::{
    ensure_selected_service_inactive, wait_until_no_conflicting_live_daemon,
    wait_until_selected_service_inactive, STOP_QUIESCENCE_TIMEOUT,
};
pub use stop::stop_active_daemon;

#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
