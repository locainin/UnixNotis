//! Interaction policy wire and matrix regressions

use zbus::zvariant::{serialized::Context, to_bytes, Type, LE};

use super::{ApplicationActionPolicy, InlineReplyPolicy, InteractionPolicies};

#[test]
fn interaction_policy_enums_keep_stable_one_byte_wire_values() {
    let context = Context::new_dbus(LE, 0);
    for (policy, discriminant) in [
        (ApplicationActionPolicy::Allow, 0_u8),
        (ApplicationActionPolicy::Confirm, 1),
        (ApplicationActionPolicy::Deny, 2),
    ] {
        let encoded = to_bytes(context, &policy).expect("serialize action policy");
        assert_eq!(ApplicationActionPolicy::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
    }
    for (policy, discriminant) in [
        (InlineReplyPolicy::Allow, 0_u8),
        (InlineReplyPolicy::Confirm, 1),
        (InlineReplyPolicy::Deny, 2),
    ] {
        let encoded = to_bytes(context, &policy).expect("serialize reply policy");
        assert_eq!(InlineReplyPolicy::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
    }
}

#[test]
fn native_compatibility_keeps_default_activation_without_richer_authority() {
    assert_eq!(
        InteractionPolicies::NATIVE_COMPATIBILITY.default_activation,
        ApplicationActionPolicy::Allow
    );
    assert_eq!(
        InteractionPolicies::NATIVE_COMPATIBILITY.action_buttons,
        ApplicationActionPolicy::Confirm
    );
    assert_eq!(
        InteractionPolicies::NATIVE_COMPATIBILITY.inline_reply,
        InlineReplyPolicy::Deny
    );
}

#[test]
fn owner_bound_default_grants_only_default_activation() {
    assert_eq!(
        InteractionPolicies::OWNER_BOUND_DEFAULT.default_activation,
        ApplicationActionPolicy::Allow
    );
    assert_eq!(
        InteractionPolicies::OWNER_BOUND_DEFAULT.action_buttons,
        ApplicationActionPolicy::Deny
    );
    assert_eq!(
        InteractionPolicies::OWNER_BOUND_DEFAULT.inline_reply,
        InlineReplyPolicy::Deny
    );
}

#[test]
fn confirmation_and_denial_matrices_never_allow_inline_text() {
    for policies in [
        InteractionPolicies::CONFIRM_ACTIONS,
        InteractionPolicies::DENY,
    ] {
        assert_ne!(
            policies.inline_reply,
            InlineReplyPolicy::Allow,
            "weaker associations must not expose credential-like reply text"
        );
    }
}
