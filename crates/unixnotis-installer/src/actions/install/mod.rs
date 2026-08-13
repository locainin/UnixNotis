//! Install and uninstall actions for binaries and service-manager artifacts

// Binary copy and cleanup live apart from service management so filesystem writes stay focused
mod binaries;
mod install_state;
mod installation_channel;
mod installer_lock;
// Service artifact writes and startup behavior stay together because they share
// service-manager state
mod service;

pub use binaries::{install_binaries, remove_binaries};
pub use install_state::{check_install_state, InstallState, InstallationDisposition};
pub(super) use installation_channel::reject_conflicting_installation_channel;
pub use installer_lock::InstallerLock;
pub use service::uninstall_service;
pub use service::write_service_artifact;
pub use service::{enforce_service_readiness, rollback_failed_activation};
pub use service::{
    install_service_under_reservation, prepare_service_start_under_reservation,
    restart_previous_service, rollback_pending_under_activation_reservation,
    start_service_and_verify,
};

#[cfg(test)]
mod tests;
