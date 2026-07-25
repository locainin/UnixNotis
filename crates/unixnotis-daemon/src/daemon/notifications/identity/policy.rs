//! Interaction decisions kept independent from presentation association

use unixnotis_core::{AttributionClass, InlineReplyPolicy};

pub(super) const fn inline_reply_policy(class: AttributionClass) -> InlineReplyPolicy {
    match class {
        AttributionClass::SystemAssociated | AttributionClass::PortalAssociated => {
            InlineReplyPolicy::Allow
        }
        AttributionClass::UserAssociated => InlineReplyPolicy::Confirm,
        AttributionClass::TrustedRelay | AttributionClass::Unknown | AttributionClass::Conflict => {
            InlineReplyPolicy::Deny
        }
    }
}
