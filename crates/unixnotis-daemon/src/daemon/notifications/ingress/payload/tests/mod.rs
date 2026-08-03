use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use zbus::zvariant::OwnedValue;

pub(super) use super::super::super::identity::SenderMetadata;
pub(super) use super::super::limits::{MAX_ACTIONS, MAX_BODY_BYTES, MAX_SUMMARY_BYTES};
pub(super) use super::build::{build_notification, NotificationInput};
pub(super) use super::expiration::resolve_expiration;
pub(super) use super::sanitize::{
    owned_to_string, parse_actions, parse_urgency_hint, sanitize_hints_for_storage,
    string_to_owned_value,
};
pub(super) use super::visuals::{
    avatar_buffer_size_allowed, avatar_file_size_allowed, bounded_decode_dimension,
    materialize_sender_visual, may_materialize_application_icon, sender_visual_file_allowed,
    MAX_SENDER_VISUAL_BYTES,
};
pub(super) use super::visuals::{sender_visual_role, SenderVisualRole};

pub(super) use unixnotis_core::{
    ApplicationActionPolicy, AttributionReason, Config, IdentityAssurance, InteractionPolicies,
    NotificationImage, Urgency,
};

mod build;
mod expiration;
mod sanitize;
mod visuals;
