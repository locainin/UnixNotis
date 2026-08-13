//! Shared service-manager identity and user-path resolution

mod envdir;
mod environment;
mod kind;
mod paths;

pub use environment::{
    validate_session_bus_address, variables_for_backend, SessionBusAddressError,
};
pub use kind::ServiceManagerKind;
pub use paths::{
    dinit_user_dir, resolve_service_manager_paths, runit_user_dir, s6_live_dir, s6_user_dir,
    systemd_user_dir, ServiceManagerPathError, ServiceManagerPaths,
};

#[cfg(test)]
mod tests;
pub use envdir::envdir_file_contents;
