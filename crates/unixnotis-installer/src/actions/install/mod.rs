//! Install and uninstall actions for binaries and systemd service files

// Binary copy and cleanup live apart from service management so filesystem writes stay focused
mod binaries;
// Service unit writes and startup behavior stay together because they share systemd state
mod service;

pub(crate) use binaries::{install_binaries, remove_binaries};
pub(crate) use service::{enable_service, install_service, uninstall_service};

#[cfg(test)]
mod tests;
