//! Install and uninstall actions for binaries and service-manager artifacts

// Binary copy and cleanup live apart from service management so filesystem writes stay focused
mod binaries;
// Service artifact writes and startup behavior stay together because they share
// service-manager state
mod service;

pub use binaries::{install_binaries, remove_binaries};
pub use service::write_service_artifact;
pub use service::{enable_service, install_service, uninstall_service};

#[cfg(test)]
mod tests;
