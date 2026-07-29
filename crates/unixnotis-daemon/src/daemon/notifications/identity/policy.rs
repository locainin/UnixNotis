//! Interaction decisions kept independent from presentation association

use unixnotis_core::{AttributionStatus, InlineReplyPolicy};

pub(super) const fn inline_reply_policy(status: AttributionStatus) -> InlineReplyPolicy {
    // Text entry stays disabled unless strong evidence identifies the application
    match status {
        AttributionStatus::Verified => InlineReplyPolicy::Allow,
        AttributionStatus::Recognized
        | AttributionStatus::Unresolved
        | AttributionStatus::Conflict
        | AttributionStatus::Relay => InlineReplyPolicy::Deny,
    }
}

#[cfg(test)]
#[path = "tests/policy.rs"]
mod tests;
