//! Interaction decisions kept independent from presentation association

use unixnotis_core::{InlineReplyPolicy, InteractionPolicies};

pub(super) const fn inline_reply_policy(interactions: InteractionPolicies) -> InlineReplyPolicy {
    // The resolver owns this policy instead of deriving text authority from branding
    interactions.inline_reply
}

#[cfg(test)]
#[path = "tests/policy.rs"]
mod tests;
