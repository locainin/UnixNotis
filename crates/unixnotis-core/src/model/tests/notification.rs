use std::collections::HashMap;

use zbus::zvariant::{serialized::Context, to_bytes, Value, LE};

use super::{Notification, NotificationImage};
use crate::{
    Action, AttributionReason, AttributionStatus, ImageData, InlineReply, InlineReplyPolicy,
    NotificationAttribution, Urgency,
};

fn notification_with_image(image: NotificationImage) -> Notification {
    let mut hints = HashMap::new();
    hints.insert(
        "category".to_string(),
        Value::from("email").try_into().expect("category hint"),
    );

    Notification {
        id: 42,
        generation: 11,
        app_name: "Mail".to_string(),
        app_icon: "mail".to_string(),
        attribution: NotificationAttribution::verified(
            "Mail",
            "Mail",
            "org.example.Mail",
            "mail",
            AttributionReason::ExactSystemExecutable,
            "exact system executable",
            "system-app:org.example.Mail".to_string(),
        ),
        attribution_diagnostics: crate::AttributionDiagnostics::default(),
        summary: "Subject".to_string(),
        body: "Body".to_string(),
        actions: vec![Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        }],
        inline_reply: InlineReply::default(),
        inline_reply_policy: InlineReplyPolicy::Allow,
        hints,
        urgency: Urgency::Critical,
        category: Some("email".to_string()),
        is_transient: true,
        is_resident: true,
        suppress_popup: true,
        suppress_sound: true,
        image,
        expire_timeout: 5000,
        received_at: chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .expect("fixed notification timestamp"),
        sender_name: Some(":1.42".to_string()),
        sender_pid: Some(1234),
        sender_start_time: Some(9000),
        sender_executable: Some("/usr/bin/mail".to_string()),
    }
}

fn image_with_raw_bytes() -> NotificationImage {
    NotificationImage {
        has_image_data: true,
        image_data: ImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![1, 2, 3, 4],
        },
        visual_role: crate::NotificationVisualRole::None,
        conversation_avatar: ImageData::default(),
        image_path: "/tmp/icon.png".to_string(),
        icon_name: "mail".to_string(),
    }
}

#[test]
fn notification_view_keeps_ui_fields_and_transient_policy_flag() {
    let notification = notification_with_image(image_with_raw_bytes());

    let view = notification.to_view();

    // Live popup views keep enough information for UI actions and close policy
    assert_eq!(view.id, 42);
    assert_eq!(view.app_name, "Mail");
    assert_eq!(view.attribution.status, AttributionStatus::Verified);
    assert_eq!(view.attribution.badge_icon, "mail");
    assert_eq!(view.summary, "Subject");
    assert_eq!(view.body, "Body");
    assert_eq!(view.actions.len(), 1);
    assert_eq!(view.urgency, Urgency::Critical.as_u8());
    assert!(view.is_transient);
    assert_eq!(view.received_at_unix_seconds, 1_700_000_000);
    assert!(view.image.has_image_data);
}

#[test]
fn notification_view_round_trips_every_attribution_and_reply_policy_pair() {
    let context = Context::new_dbus(LE, 0);
    let cases = [
        (AttributionStatus::Verified, InlineReplyPolicy::Allow),
        (AttributionStatus::Recognized, InlineReplyPolicy::Deny),
        (AttributionStatus::Relay, InlineReplyPolicy::Deny),
        (AttributionStatus::Unresolved, InlineReplyPolicy::Deny),
        (AttributionStatus::Conflict, InlineReplyPolicy::Deny),
    ];

    for (status, policy) in cases {
        let mut view = notification_with_image(image_with_raw_bytes()).to_view();
        view.attribution.status = status;
        view.inline_reply_policy = policy;

        // This nested payload matches GetActiveNotification and exercises both wire enums
        let encoded = to_bytes(context, &view).expect("serialize notification view");
        let decoded = encoded
            .deserialize::<super::NotificationView>()
            .expect("deserialize notification view")
            .0;
        assert_eq!(decoded, view);
    }
}

#[test]
fn notification_view_keeps_conflict_warning_separate_from_primary_name() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.app_name = "Password Manager".to_string();
    notification.sender_executable = Some("/usr/bin/unknown-client".to_string());
    notification.attribution = NotificationAttribution::conflict(
        "Password Manager",
        "org.example.PasswordManager",
        AttributionReason::ExecutableMismatch,
        "source /usr/bin/unknown-client",
        "executable:1:2".to_string(),
    );

    let view = notification.to_view();

    assert_eq!(view.app_name, "Unknown application");
    assert_eq!(view.attribution.status, AttributionStatus::Conflict);
    assert_eq!(view.attribution.claimed_name, "Password Manager");
    assert_eq!(
        view.attribution.reason,
        AttributionReason::ExecutableMismatch
    );
    assert!(!view.app_name.contains("unverified claim"));
}

#[test]
fn notification_view_strips_markup_from_ui_text() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.summary = "<b>Crash Reporting System</b>".to_string();
    notification.body = "<html><tt>/usr/lib/drkonqi</tt> has encountered &quot;fatal&quot;<br>error &amp; closed.</html>".to_string();

    let view = notification.to_view();

    // UI labels render plain text, so notification markup must be normalized first
    assert_eq!(view.summary, "Crash Reporting System");
    assert_eq!(
        view.body,
        "/usr/lib/drkonqi has encountered \"fatal\"\nerror & closed."
    );
}

#[test]
fn notification_view_decodes_numeric_entities() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "Temperature: &#45;5&#176;C &#x26; falling".to_string();

    let view = notification.to_view();

    // Numeric entities appear in real notification bodies from markup-aware senders
    assert_eq!(view.body, "Temperature: -5°C & falling");
}

#[test]
fn notification_view_decodes_common_named_entities() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "Use &lt;tag&gt;&nbsp;and don&apos;t panic".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "Use <tag> and don't panic");
}

#[test]
fn notification_view_treats_self_closing_break_as_newline() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "Line one<br/>Line two".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "Line one\nLine two");
}

#[test]
fn notification_view_matches_block_tags_without_allocating_lowercase_names() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "Line one<BR>Line two</P>Line three".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "Line one\nLine two\nLine three");
}

#[test]
fn notification_view_preserves_inline_markup_adjacency() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "foo<b>bar</b> and <i>baz</i>".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "foobar and baz");
}

#[test]
fn notification_view_refolds_tokens_joined_by_markup_removal() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.summary = format!(
        "markup-{}",
        "<a href='https://example.test/path'>link</a>".repeat(180)
    );

    let view = notification.to_view();
    let longest = view
        .summary
        .split_whitespace()
        .map(|token| token.chars().count())
        .max()
        .unwrap_or_default();

    assert!(view.summary.contains('…'));
    assert!(longest <= crate::util::MAX_DISPLAY_TOKEN_WIDTH);
}

#[test]
fn notification_view_collapses_inline_spaces_without_leaking_after_blocks() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "Alpha  <b>Beta</b><br> Gamma".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "Alpha Beta\nGamma");
}

#[test]
fn notification_view_collapses_repeated_block_tag_newlines() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "Line one<br><br>Line two".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "Line one\nLine two");
}

#[test]
fn notification_view_removes_trailing_whitespace_in_place() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "  Alpha  \n  ".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "Alpha");
}

#[test]
fn notification_view_preserves_unterminated_entity_text() {
    let mut notification = notification_with_image(image_with_raw_bytes());
    notification.body = "Fish &chips".to_string();

    let view = notification.to_view();

    assert_eq!(view.body, "Fish &chips");
}

#[test]
fn list_view_strips_raw_image_bytes_but_keeps_icon_identifiers() {
    let notification = notification_with_image(image_with_raw_bytes());

    let view = notification.to_list_view();

    // List rows should avoid carrying raw image buffers across D-Bus
    assert!(!view.image.has_image_data);
    assert!(view.image.image_data.data.is_empty());
    assert_eq!(view.image.image_path, "/tmp/icon.png");
    assert_eq!(view.image.icon_name, "mail");
    assert!(view.is_transient);
}

#[test]
fn history_projection_drops_raw_hints_and_image_bytes() {
    let notification = notification_with_image(image_with_raw_bytes());

    let history = notification.to_history();

    // History entries should stay lightweight and avoid retaining raw D-Bus hints
    assert!(history.hints.is_empty());
    assert!(!history.image.has_image_data);
    assert!(history.image.image_data.data.is_empty());
    assert_eq!(history.sender_name.as_deref(), Some(":1.42"));
    assert_eq!(history.sender_pid, Some(1234));
    assert_eq!(history.sender_start_time, Some(9000));
    assert_eq!(history.sender_executable.as_deref(), Some("/usr/bin/mail"));
}
