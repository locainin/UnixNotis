//! Service artifact writes, backend refresh, lifecycle, and install flow

mod artifacts;
mod dirs;
mod files;
mod flow;
mod lifecycle;
mod refresh;
mod symlinks;

#[cfg(test)]
pub(in crate::actions::install) use artifacts::remove_service_artifact;
pub use artifacts::write_service_artifact;
#[cfg(test)]
pub(in crate::actions::install) use files::{current_mode, ensure_regular_artifact_file_path};
pub use flow::{enable_service, install_service, uninstall_service};
#[cfg(test)]
pub(in crate::actions::install) use lifecycle::{
    service_start_mode_from_enabled, ServiceStartMode,
};
#[cfg(test)]
pub(in crate::actions::install) use refresh::{
    s6_stderr_diagnostic, sanitize_diagnostic_line, strip_ansi_csi_sequences, truncate_diagnostic,
};
#[cfg(test)]
pub(in crate::actions::install) use symlinks::{remove_service_symlink, write_service_symlink};
