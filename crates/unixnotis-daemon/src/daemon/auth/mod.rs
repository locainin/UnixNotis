//! Authorization helpers for privileged control methods
//!
//! The control D-Bus interface calls into this module before mutating daemon
//! state. The implementation is split by responsibility because each layer has
//! a different security job: caller identity, executable path trust, metadata
//! checks, and startup-time fingerprint pinning
//!
//! This is defense in depth for the desktop session, not isolation from hostile
//! code already running as the same uid
//! Same-user code may inherit or deliberately share a bus connection, then exec
//! another program while retaining control of that connection
//! A hard same-user boundary requires kernel-enforced separation such as distinct
//! users, a constrained broker, or an LSM policy

mod authorization;
mod credentials;
mod executable_trust;
mod policy;
mod process_identity;

pub(super) use authorization::{
    authorize_control_call, authorize_interaction_call, authorize_panel_readiness_call,
    authorize_popup_readiness_call,
};
pub(in crate::daemon) use executable_trust::build_trusted_control_snapshots_for_current_executable;
pub(in crate::daemon) use policy::TrustedExecutableSnapshot;

#[cfg(test)]
#[path = "tests/authorization.rs"]
mod authorization_tests;
#[cfg(test)]
#[path = "tests/credentials.rs"]
mod credentials_tests;
#[cfg(test)]
#[path = "tests/process_identity.rs"]
mod process_identity_tests;
#[cfg(test)]
#[path = "tests/support.rs"]
mod support;
