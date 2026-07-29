use unixnotis_core::{Action, AttributionReason, InlineReplyPolicy, NotificationAttribution};
use unixnotis_ui::presentation::TrustLevel;

use super::super::{PopupTrustPresentation, ReplyPresentation};
use super::support::notification;

#[test]
fn protected_desktop_association_stays_verified_and_visually_quiet() {
    let mut view = notification();
    view.inline_reply.available = true;
    view.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::Verified);
    assert!(trust.short_label.is_none());
    assert_eq!(trust.reply, ReplyPresentation::Available);
}

#[test]
fn trusted_relay_uses_human_source_text_and_keeps_raw_path_in_details() {
    let mut view = notification();
    view.attribution = NotificationAttribution::relay(
        "Screenshot",
        "Sent via /usr/bin/notify-send",
        "relay:screenshot".to_string(),
    );
    view.inline_reply_policy = InlineReplyPolicy::Deny;

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::Relay);
    assert!(trust.short_label.is_none());
    assert_eq!(
        trust.details_label.as_deref(),
        Some("Sent via /usr/bin/notify-send")
    );
    assert_eq!(trust.reply, ReplyPresentation::Hidden);
}

#[test]
fn conflicting_claim_is_suspicious_and_cannot_enable_reply() {
    let mut view = notification();
    view.attribution = NotificationAttribution::conflict(
        "Signal",
        "org.signal.Signal",
        AttributionReason::ExecutableMismatch,
        "source /tmp/fake",
        "conflict:signal".to_string(),
    );
    view.inline_reply.available = true;
    view.inline_reply_policy = InlineReplyPolicy::Deny;

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::Conflict);
    assert_eq!(trust.short_label.as_deref(), Some("Suspicious"));
    assert_eq!(trust.reply, ReplyPresentation::Unavailable);
}

#[test]
fn user_writable_desktop_association_remains_unverified() {
    let mut view = notification();
    view.attribution = NotificationAttribution::recognized(
        "Local app",
        "Local app",
        "org.example.Local",
        "local-app",
        AttributionReason::ExactUserExecutable,
        "user desktop association",
        "user-desktop:org.example.Local".to_string(),
    );

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::Recognized);
    assert_eq!(trust.short_label.as_deref(), Some("Unverified"));
    assert_eq!(trust.reply, ReplyPresentation::Hidden);
}

#[test]
fn verified_identity_still_needs_both_a_reply_request_and_policy_permission() {
    let mut denied = notification();
    denied.inline_reply.available = true;
    denied.inline_reply_policy = InlineReplyPolicy::Deny;
    let denied_trust = PopupTrustPresentation::for_notification(&denied);
    assert_eq!(denied_trust.reply, ReplyPresentation::Unavailable);

    let no_request = notification();
    let no_request_trust = PopupTrustPresentation::for_notification(&no_request);
    assert_eq!(no_request_trust.reply, ReplyPresentation::Hidden);
}

#[test]
fn only_the_exact_inline_reply_action_key_requests_reply_ui() {
    let mut other_action = notification();
    other_action.actions.push(Action {
        key: "reply-later".to_string(),
        label: "Reply later".to_string(),
    });
    let other_trust = PopupTrustPresentation::for_notification(&other_action);
    assert_eq!(other_trust.reply, ReplyPresentation::Hidden);

    let mut inline_reply = notification();
    inline_reply.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let inline_trust = PopupTrustPresentation::for_notification(&inline_reply);
    assert_eq!(inline_trust.reply, ReplyPresentation::Unavailable);
}
