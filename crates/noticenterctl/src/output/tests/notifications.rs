use unixnotis_core::{Action, NotificationImage, NotificationView};

use super::{format_inhibitors, format_notifications};
use crate::output::{allow_full_output, warn_full_requires_diagnostic};

fn sample_notification() -> NotificationView {
    // Bad bytes on purpose
    NotificationView {
        id: 7,
        app_name: "mailer\n\x1b[31m".to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            display_name: "mailer\n\x1b[31m".to_string(),
            badge_icon: "mailer".to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: "subject\rline".to_string(),
        body: "body\ttext\nnext".to_string(),
        actions: vec![Action {
            key: "open".to_string(),
            label: "Open".to_string(),
        }],
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        is_transient: false,
        // CLI formatting only needs the lightweight transport fields
        image: NotificationImage::default(),
    }
}

#[test]
fn format_notifications_sanitizes_terminal_control_sequences() {
    // Compact output stays clean
    let output = format_notifications("active", &[sample_notification()], false);
    assert!(output.contains("mailer"));
    assert!(output.contains("[31m]"));
    assert!(output.contains("subject line"));
    assert!(!output.contains('\n') || output.lines().count() == 2);
    assert!(!output.contains('\u{1b}'));
}

#[test]
fn format_notifications_full_mode_includes_body() {
    // Full mode prints the body
    let output = format_notifications("history", &[sample_notification()], true);
    assert!(output.contains("body: body text next"));
}

#[test]
fn format_inhibitors_sanitizes_reason_and_owner() {
    // Both fields print straight to the terminal
    let output = format_inhibitors(&[(5, "present\nmode".to_string(), 1, ":1.2\r".to_string())]);
    assert!(output.contains("owner=:1.2 "));
    assert!(output.contains("reason=present mode"));
}

#[test]
fn full_output_requires_request_and_diagnostic_mode() {
    assert!(allow_full_output(true, true));
    assert!(!allow_full_output(true, false));
    assert!(!allow_full_output(false, true));
    assert!(!allow_full_output(false, false));
}

#[test]
fn full_output_warning_only_when_full_was_requested_without_diagnostic_mode() {
    assert!(warn_full_requires_diagnostic(true, false));
    assert!(!warn_full_requires_diagnostic(true, true));
    assert!(!warn_full_requires_diagnostic(false, false));
    assert!(!warn_full_requires_diagnostic(false, true));
}
