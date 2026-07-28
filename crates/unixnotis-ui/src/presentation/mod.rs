//! Shared notification presentation decisions for popup and panel clients

mod badges;
mod build;
mod text;
mod types;

pub use badges::{build_semantic_badge, register_semantic_badges};
pub use build::NotificationPresentation;
pub use text::{
    clamp_label_text, has_visible_text, ACTION_LABEL_MAX_CHARS, APP_LABEL_MAX_CHARS,
    BODY_LABEL_MAX_CHARS, SUMMARY_LABEL_MAX_CHARS,
};
pub use types::{
    ActionPresentation, ActionView, BadgePresentation, IdentityPresentation, MediaPresentation,
    NotificationKind, ReplyPresentation, ThumbnailKind, TrustLevel, TrustPresentation,
};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
