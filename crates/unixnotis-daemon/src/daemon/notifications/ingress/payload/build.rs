//! Convert bounded wire fields into a stored notification

use std::collections::HashMap;

use unixnotis_core::{
    util, Action, AttributionDiagnostics, ImageData, InlineReply, InlineReplyPolicy, Notification,
    NotificationAttribution, NotificationImage, NotificationVisualRole, Urgency,
};
use zbus::zvariant::OwnedValue;

use super::super::super::identity::SenderMetadata;
use super::super::limits::{
    MAX_APP_ICON_BYTES, MAX_APP_NAME_BYTES, MAX_BODY_BYTES, MAX_CATEGORY_BYTES, MAX_SUMMARY_BYTES,
};
use super::sanitize::parse_actions;
use super::visuals::{may_materialize_application_icon, SenderVisualRole};
use super::{owned_to_string, sanitize_hints_for_storage};

pub(in crate::daemon::notifications) struct NotificationInput {
    pub(in crate::daemon::notifications) app_name: String,
    pub(in crate::daemon::notifications) app_icon: String,
    pub(in crate::daemon::notifications) summary: String,
    pub(in crate::daemon::notifications) body: String,
    pub(in crate::daemon::notifications) actions: Vec<String>,
    pub(in crate::daemon::notifications) hints: HashMap<String, OwnedValue>,
    pub(in crate::daemon::notifications) image_data: Option<ImageData>,
    pub(in crate::daemon::notifications) sender_visual: Option<ImageData>,
    pub(in crate::daemon::notifications) sender_visual_role: SenderVisualRole,
    pub(in crate::daemon::notifications) sender: SenderMetadata,
    pub(in crate::daemon::notifications) attribution: NotificationAttribution,
    pub(in crate::daemon::notifications) attribution_diagnostics: AttributionDiagnostics,
    pub(in crate::daemon::notifications) inline_reply_policy: InlineReplyPolicy,
    pub(in crate::daemon::notifications) expire_timeout: i32,
}

pub(in crate::daemon::notifications) fn build_notification(
    input: NotificationInput,
) -> Notification {
    let NotificationInput {
        app_name,
        app_icon,
        summary,
        body,
        actions,
        hints,
        image_data,
        sender_visual,
        sender_visual_role,
        sender,
        attribution,
        attribution_diagnostics,
        inline_reply_policy,
        expire_timeout,
    } = input;

    let urgency = Urgency::from_hint(hints.get("urgency"));
    let category = hints
        .get("category")
        .and_then(owned_to_string)
        .map(|value| {
            util::truncate_utf8_bytes(
                &util::sanitize_inline_display_text(&value),
                MAX_CATEGORY_BYTES,
            )
        });
    let is_transient = hints
        .get("transient")
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false);
    let is_resident = hints
        .get("resident")
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false);
    let image = build_image(
        &app_name,
        &app_icon,
        &hints,
        image_data,
        sender_visual,
        sender_visual_role,
        &attribution,
    );

    let actions = parse_actions(actions);
    let inline_reply = parse_inline_reply(&actions, &hints);
    let app_name = util::sanitize_inline_display_text(&app_name);
    let summary = util::sanitize_display_text(&summary);
    let body = util::sanitize_display_text(&body);

    Notification {
        id: 0,
        generation: 0,
        app_name: if app_name.is_empty() {
            "Unknown".to_string()
        } else {
            util::truncate_utf8_bytes(&app_name, MAX_APP_NAME_BYTES)
        },
        app_icon: if super::visuals::local_avatar_path(&app_icon).is_some() {
            String::new()
        } else {
            util::truncate_utf8_bytes(&app_icon, MAX_APP_ICON_BYTES)
        },
        attribution,
        attribution_diagnostics,
        summary: util::fold_text_for_layout(
            &util::truncate_utf8_bytes(&summary, MAX_SUMMARY_BYTES),
            util::MAX_DISPLAY_TOKEN_WIDTH,
        ),
        body: util::fold_text_for_layout(
            &util::truncate_utf8_bytes(&body, MAX_BODY_BYTES),
            util::MAX_DISPLAY_TOKEN_WIDTH,
        ),
        actions,
        inline_reply,
        inline_reply_policy,
        hints: sanitize_hints_for_storage(hints),
        urgency,
        category,
        is_transient,
        is_resident,
        suppress_popup: false,
        suppress_sound: false,
        image,
        expire_timeout,
        received_at: chrono::Utc::now(),
        sender_name: sender.sender_name,
        sender_pid: sender.sender_pid,
        sender_start_time: sender.sender_start_time,
        sender_executable: sender.sender_executable,
    }
}

fn build_image(
    app_name: &str,
    app_icon: &str,
    hints: &HashMap<String, OwnedValue>,
    image_data: Option<ImageData>,
    sender_visual: Option<ImageData>,
    sender_visual_role: SenderVisualRole,
    attribution: &NotificationAttribution,
) -> NotificationImage {
    // Keep daemon-selected badge identity separate from sender-provided pixels
    let mut image = NotificationImage::from_hints(app_name, app_icon, hints);
    image.badge_icon.clone_from(&attribution.badge_icon);
    if let Some(image_data) = image_data.and_then(NotificationImage::normalize_image_data) {
        image.content_image = image_data;
    }
    if may_materialize_application_icon(attribution) {
        if let Some(visual) = sender_visual.and_then(NotificationImage::normalize_image_data) {
            image.sender_visual_role = match sender_visual_role {
                SenderVisualRole::ConversationAvatar => NotificationVisualRole::ConversationAvatar,
                SenderVisualRole::ApplicationProvidedIcon => {
                    NotificationVisualRole::ApplicationProvidedIcon
                }
                SenderVisualRole::None => NotificationVisualRole::None,
            };
            image.sender_visual = visual;
        }
    }
    image
}

fn parse_inline_reply(actions: &[Action], hints: &HashMap<String, OwnedValue>) -> InlineReply {
    let Some(action) = actions.iter().find(|action| action.key == "inline-reply") else {
        return InlineReply::default();
    };

    InlineReply {
        available: true,
        label: action.label.clone(),
        placeholder: reply_hint_text(hints, "x-kde-reply-placeholder-text"),
        submit_label: reply_hint_text(hints, "x-kde-reply-submit-button-text"),
        submit_icon: reply_hint_text(hints, "x-kde-reply-submit-button-icon-name"),
    }
}

fn reply_hint_text(hints: &HashMap<String, OwnedValue>, key: &str) -> String {
    let Some(value) = hints.get(key).and_then(owned_to_string) else {
        return String::new();
    };
    let clean = util::sanitize_inline_display_text(&value);
    util::truncate_utf8_bytes(&clean, super::super::limits::MAX_HINT_STRING_BYTES)
}
