//! Independent authority for application-owned notification controls

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

/// Policy for credential-like inline text controls
#[derive(Debug, Copy, Clone, Default, Serialize_repr, Deserialize_repr, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum InlineReplyPolicy {
    Allow = 0,
    Confirm = 1,
    #[default]
    Deny = 2,
}

/// Policy for one application-owned action signal
#[derive(Debug, Copy, Clone, Default, Serialize_repr, Deserialize_repr, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum ApplicationActionPolicy {
    Allow = 0,
    Confirm = 1,
    #[default]
    Deny = 2,
}

/// Independent authority for each interaction surface
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct InteractionPolicies {
    pub default_activation: ApplicationActionPolicy,
    pub action_buttons: ApplicationActionPolicy,
    pub inline_reply: InlineReplyPolicy,
}

impl InteractionPolicies {
    /// Future strong boundaries may grant every advertised interaction
    pub const AUTHENTICATED: Self = Self {
        default_activation: ApplicationActionPolicy::Allow,
        action_buttons: ApplicationActionPolicy::Allow,
        inline_reply: InlineReplyPolicy::Allow,
    };

    /// Native association keeps compatible card activation but gates richer controls
    pub const NATIVE_COMPATIBILITY: Self = Self {
        default_activation: ApplicationActionPolicy::Allow,
        action_buttons: ApplicationActionPolicy::Confirm,
        inline_reply: InlineReplyPolicy::Deny,
    };

    /// Brokered and user-local associations require confirmation for every action
    pub const CONFIRM_ACTIONS: Self = Self {
        default_activation: ApplicationActionPolicy::Confirm,
        action_buttons: ApplicationActionPolicy::Confirm,
        inline_reply: InlineReplyPolicy::Deny,
    };

    /// Uncertain or contradictory senders cannot emit application-owned signals
    pub const DENY: Self = Self {
        default_activation: ApplicationActionPolicy::Deny,
        action_buttons: ApplicationActionPolicy::Deny,
        inline_reply: InlineReplyPolicy::Deny,
    };
}

#[cfg(test)]
#[path = "tests/interaction.rs"]
mod tests;
