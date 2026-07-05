//! Output formatting helpers for noticenterctl

use unixnotis_core::{util, NotificationView};

pub(crate) fn print_notifications(label: &str, notifications: &[NotificationView], full: bool) {
    // One place for CLI output
    print!("{}", format_notifications(label, notifications, full));
}

pub(crate) fn print_inhibitors(inhibitors: &[(u64, String, u32, String)]) {
    // Same output path for inhibitor rows
    print!("{}", format_inhibitors(inhibitors));
}

fn format_notifications(label: &str, notifications: &[NotificationView], full: bool) -> String {
    // Respect the diagnostic mode guard so secrets are not printed unintentionally
    let limit = if full {
        util::diagnostic_log_limit()
    } else {
        util::default_log_limit()
    };

    // Build the whole payload first so tests can assert exact CLI output
    let mut out = String::new();
    out.push_str(&format!("{label} notifications: {}\n", notifications.len()));

    for notification in notifications {
        // Keep app names to one line
        let app = util::sanitize_log_value(&notification.app_name, limit);
        // Keep summaries safe too
        let summary = util::sanitize_log_value(&notification.summary, limit);
        // Short view only shows the count
        let action_count = notification.actions.len();
        out.push_str(&format!(
            "- #{id} [{app}] {summary} (actions={actions})\n",
            id = notification.id,
            app = app,
            summary = summary,
            actions = action_count
        ));

        if full {
            // Body is still cleaned before print
            let body = util::sanitize_log_value(&notification.body, limit);
            out.push_str(&format!("  body: {body}\n"));
        }
    }

    out
}

fn format_inhibitors(inhibitors: &[(u64, String, u32, String)]) -> String {
    // Default log limit is enough here because inhibitor rows are operational metadata
    let limit = util::default_log_limit();
    let mut out = String::new();
    out.push_str(&format!("inhibitors: {}\n", inhibitors.len()));

    for (id, reason, scope, owner) in inhibitors {
        // Owner comes from outside
        let owner = util::sanitize_log_value(owner, limit);
        // Reason comes from outside too
        let reason = util::sanitize_log_value(reason, limit);
        out.push_str(&format!(
            "- #{id} scope={scope} owner={owner} reason={reason}\n"
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use unixnotis_core::{Action, NotificationImage, NotificationView};

    use super::{format_inhibitors, format_notifications};

    fn sample_notification() -> NotificationView {
        // Bad bytes on purpose
        NotificationView {
            id: 7,
            app_name: "mailer\n\x1b[31m".to_string(),
            summary: "subject\rline".to_string(),
            body: "body\ttext\nnext".to_string(),
            actions: vec![Action {
                key: "open".to_string(),
                label: "Open".to_string(),
            }],
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
        let output =
            format_inhibitors(&[(5, "present\nmode".to_string(), 1, ":1.2\r".to_string())]);
        assert!(output.contains("owner=:1.2 "));
        assert!(output.contains("reason=present mode"));
    }
}
