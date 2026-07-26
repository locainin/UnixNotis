use unixnotis_core::{AttributionClass, InlineReplyPolicy};

use super::inline_reply_policy;

#[test]
fn only_system_and_portal_associations_allow_inline_replies() {
    for class in [
        AttributionClass::SystemAssociated,
        AttributionClass::PortalAssociated,
    ] {
        assert_eq!(inline_reply_policy(class), InlineReplyPolicy::Allow);
    }
}

#[test]
fn every_unconfirmed_attribution_class_denies_inline_replies() {
    for class in [
        AttributionClass::UserAssociated,
        AttributionClass::TrustedRelay,
        AttributionClass::Unknown,
        AttributionClass::Conflict,
    ] {
        assert_eq!(inline_reply_policy(class), InlineReplyPolicy::Deny);
    }
}
