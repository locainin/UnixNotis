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
pub use flow::{enable_service, install_service, uninstall_service};
