//! Shared service-manager identity and user-path resolution

mod envdir;
mod kind;
mod paths;

pub use kind::ServiceManagerKind;
pub use paths::{
    dinit_user_dir, resolve_service_manager_paths, runit_user_dir, s6_live_dir, s6_user_dir,
    systemd_user_dir, ServiceManagerPathError, ServiceManagerPaths,
};

#[cfg(test)]
mod tests;
pub use envdir::envdir_file_contents;
