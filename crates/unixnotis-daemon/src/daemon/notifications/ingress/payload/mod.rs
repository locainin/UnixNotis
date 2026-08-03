//! Bounded notification payload construction

mod build;
mod expiration;
mod sanitize;
mod visuals;

pub(in crate::daemon::notifications) use build::{build_notification, NotificationInput};
pub(in crate::daemon::notifications) use expiration::resolve_expiration;
pub(in crate::daemon::notifications) use sanitize::{owned_to_string, sanitize_hints_for_storage};
pub(in crate::daemon::notifications) use visuals::{
    materialize_sender_visual, may_materialize_content_image, sender_visual_role, SenderVisualRole,
    CONVERSATION_AVATAR_TIMEOUT, MAX_STORED_CONTENT_DIMENSION,
};

#[cfg(test)]
mod tests;
