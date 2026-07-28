use unixnotis_core::{Action, AttributionClass, InlineReplyPolicy, NotificationAttribution};

use super::super::{PopupTrustPresentation, TrustLevel};
use super::support::notification;

#[test]
fn protected_desktop_association_stays_verified_and_visually_quiet() {
    let mut view = notification();
    view.inline_reply.available = true;

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::Verified);
    assert!(trust.short_label.is_none());
    assert!(trust.allow_reply);
}

#[test]
fn trusted_relay_uses_human_source_text_and_keeps_raw_path_in_details() {
    let mut view = notification();
    view.attribution = NotificationAttribution::trusted_relay(
        "Screenshot",
        "Sent via /usr/bin/notify-send",
        false,
        "relay:screenshot".to_string(),
    );
    view.inline_reply_policy = InlineReplyPolicy::Deny;

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::System);
    assert_eq!(trust.short_label.as_deref(), Some("Command-line tool"));
    assert_eq!(
        trust.details_label.as_deref(),
        Some("Sent via /usr/bin/notify-send")
    );
    assert!(!trust.allow_reply);
}

#[test]
fn conflicting_claim_is_suspicious_and_cannot_enable_reply() {
    let mut view = notification();
    view.attribution = NotificationAttribution::conflict(
        "Signal",
        "source /tmp/fake",
        "conflict:signal".to_string(),
    );
    view.inline_reply.available = true;
    view.inline_reply_policy = InlineReplyPolicy::Deny;

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::Suspicious);
    assert_eq!(trust.short_label.as_deref(), Some("Suspicious"));
    assert!(!trust.allow_reply);
    assert!(trust.show_reply_unavailable);
}

#[test]
fn user_writable_desktop_association_remains_unverified() {
    let mut view = notification();
    view.attribution = NotificationAttribution::associated(
        "Local app",
        "org.example.Local",
        "org.example.Local",
        "user desktop association",
        AttributionClass::UserAssociated,
        false,
        "user-desktop:org.example.Local".to_string(),
    );

    let trust = PopupTrustPresentation::for_notification(&view);

    assert_eq!(trust.level, TrustLevel::Unverified);
    assert_eq!(trust.short_label.as_deref(), Some("Unverified"));
    assert!(!trust.show_reply_unavailable);
}

#[test]
fn verified_identity_still_needs_both_a_reply_request_and_policy_permission() {
    let mut denied = notification();
    denied.inline_reply.available = true;
    denied.inline_reply_policy = InlineReplyPolicy::Deny;
    let denied_trust = PopupTrustPresentation::for_notification(&denied);
    assert!(!denied_trust.allow_reply);
    assert!(denied_trust.show_reply_unavailable);

    let no_request = notification();
    let no_request_trust = PopupTrustPresentation::for_notification(&no_request);
    assert!(!no_request_trust.allow_reply);
    assert!(!no_request_trust.show_reply_unavailable);
}

#[test]
fn only_the_exact_inline_reply_action_key_requests_reply_ui() {
    let mut other_action = notification();
    other_action.actions.push(Action {
        key: "reply-later".to_string(),
        label: "Reply later".to_string(),
    });
    let other_trust = PopupTrustPresentation::for_notification(&other_action);
    assert!(!other_trust.allow_reply);
    assert!(!other_trust.show_reply_unavailable);

    let mut inline_reply = notification();
    inline_reply.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let inline_trust = PopupTrustPresentation::for_notification(&inline_reply);
    assert!(inline_trust.allow_reply);
    assert!(!inline_trust.show_reply_unavailable);
}
