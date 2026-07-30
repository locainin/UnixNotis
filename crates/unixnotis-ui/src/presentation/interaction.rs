//! Shared confirmation state for application-owned controls

use unixnotis_core::ApplicationActionPolicy;

/// Result of one user activation attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionActivation {
    Denied,
    ArmConfirmation,
    Invoke { confirmed: bool },
}

/// Convert policy and local confirmation state into one safe UI action
#[must_use]
pub const fn action_activation(
    policy: ApplicationActionPolicy,
    confirmation_armed: bool,
) -> ActionActivation {
    match (policy, confirmation_armed) {
        (ApplicationActionPolicy::Allow, _) => ActionActivation::Invoke { confirmed: false },
        (ApplicationActionPolicy::Confirm, false) => ActionActivation::ArmConfirmation,
        (ApplicationActionPolicy::Confirm, true) => ActionActivation::Invoke { confirmed: true },
        (ApplicationActionPolicy::Deny, _) => ActionActivation::Denied,
    }
}
