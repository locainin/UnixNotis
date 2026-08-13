//! Application action confirmation regressions

use unixnotis_core::ApplicationActionPolicy;

use super::super::{action_activation, ActionActivation};

#[test]
fn allowed_action_invokes_without_confirmation() {
    assert_eq!(
        action_activation(ApplicationActionPolicy::Allow, false),
        ActionActivation::Invoke { confirmed: false },
        "allowed actions should invoke without adding confirmation state"
    );
}

#[test]
fn confirm_action_requires_two_activation_attempts() {
    assert_eq!(
        action_activation(ApplicationActionPolicy::Confirm, false),
        ActionActivation::ArmConfirmation,
        "the first activation should only arm confirmation"
    );
    assert_eq!(
        action_activation(ApplicationActionPolicy::Confirm, true),
        ActionActivation::Invoke { confirmed: true },
        "the second activation should carry explicit confirmation"
    );
}

#[test]
fn denied_action_never_invokes_even_when_armed() {
    for armed in [false, true] {
        assert_eq!(
            action_activation(ApplicationActionPolicy::Deny, armed),
            ActionActivation::Denied,
            "denied actions must not inherit stale confirmation state"
        );
    }
}
