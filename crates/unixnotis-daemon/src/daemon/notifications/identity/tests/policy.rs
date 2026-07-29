use unixnotis_core::{AttributionStatus, InlineReplyPolicy};

use super::inline_reply_policy;

#[test]
fn only_system_and_portal_associations_allow_inline_replies() {
    for class in [AttributionStatus::Verified, AttributionStatus::Verified] {
        assert_eq!(inline_reply_policy(class), InlineReplyPolicy::Allow);
    }
}

#[test]
fn every_unconfirmed_attribution_class_denies_inline_replies() {
    for class in [
        AttributionStatus::Recognized,
        AttributionStatus::Relay,
        AttributionStatus::Unresolved,
        AttributionStatus::Conflict,
    ] {
        assert_eq!(inline_reply_policy(class), InlineReplyPolicy::Deny);
    }
}
