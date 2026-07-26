//! Interaction decisions kept independent from presentation association

use unixnotis_core::{AttributionClass, InlineReplyPolicy};

pub(super) const fn inline_reply_policy(class: AttributionClass) -> InlineReplyPolicy {
    // Text entry stays disabled unless system or portal evidence identifies the application
    match class {
        AttributionClass::SystemAssociated | AttributionClass::PortalAssociated => {
            InlineReplyPolicy::Allow
        }
        AttributionClass::UserAssociated
        | AttributionClass::TrustedRelay
        | AttributionClass::Unknown
        | AttributionClass::Conflict => InlineReplyPolicy::Deny,
    }
}

#[cfg(test)]
#[path = "tests/policy.rs"]
mod tests;
