use unixnotis_core::{InlineReplyPolicy, InteractionPolicies};

use super::inline_reply_policy;

#[test]
fn only_authenticated_policy_allows_inline_replies() {
    assert_eq!(
        inline_reply_policy(InteractionPolicies::AUTHENTICATED),
        InlineReplyPolicy::Allow,
        "authenticated interaction policy should retain reply authority"
    );
}

#[test]
fn every_same_user_association_policy_denies_inline_replies() {
    for policies in [
        InteractionPolicies::NATIVE_COMPATIBILITY,
        InteractionPolicies::CONFIRM_ACTIONS,
        InteractionPolicies::DENY,
    ] {
        assert_eq!(
            inline_reply_policy(policies),
            InlineReplyPolicy::Deny,
            "same-user execution cannot authenticate credential-like reply text"
        );
    }
}
