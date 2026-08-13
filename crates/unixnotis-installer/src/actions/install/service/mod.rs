//! Service artifact writes, backend refresh, lifecycle, and install flow

pub(in crate::actions::install) mod artifacts;
mod dirs;
pub(in crate::actions::install) mod files;
pub(in crate::actions::install) mod flow;
pub(in crate::actions::install) mod lifecycle;
mod readiness;
pub(in crate::actions::install) mod refresh;
pub(in crate::actions::install) mod symlinks;

pub use artifacts::write_service_artifact;
pub use flow::install_service_under_reservation;
pub use flow::rollback_failed_activation;
pub use flow::uninstall_service;
pub use flow::{
    prepare_service_start_under_reservation, restart_previous_service,
    rollback_pending_under_activation_reservation, start_service_and_verify,
};
pub use readiness::enforce_service_readiness;
