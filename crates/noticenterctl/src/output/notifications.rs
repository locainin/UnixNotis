//! Notification and inhibitor list rendering

use unixnotis_core::{util, NotificationView};

pub fn print_notifications(
    label: &str,
    notifications: &[NotificationView],
    full: bool,
) -> anyhow::Result<()> {
    // One formatter keeps terminal output and tests on the same sanitization path
    super::write_stdout(&format_notifications(label, notifications, full))
}

pub fn print_inhibitors(inhibitors: &[(u64, String, u32, String)]) -> anyhow::Result<()> {
    // Inhibitor rows share the same bounded rendering policy
    super::write_stdout(&format_inhibitors(inhibitors))
}

fn format_notifications(label: &str, notifications: &[NotificationView], full: bool) -> String {
    // Diagnostic mode permits longer text but never bypasses control stripping
    let limit = if full {
        util::diagnostic_log_limit()
    } else {
        util::default_log_limit()
    };

    // Build one payload so partial writes cannot interleave notification rows
    let mut out = String::new();
    out.push_str(&format!("{label} notifications: {}\n", notifications.len()));

    for notification in notifications {
        // Both fields come from notification clients and must remain single-line
        let app = util::sanitize_log_value(&notification.attribution.display_name, limit);
        let summary = util::sanitize_log_value(&notification.summary, limit);
        let action_count = notification.actions.len();
        out.push_str(&format!(
            "- #{id} [{app}] {summary} (actions={actions})\n",
            id = notification.id,
            app = app,
            summary = summary,
            actions = action_count
        ));

        if full {
            // Full output adds body text without relaxing terminal safety
            let body = util::sanitize_log_value(&notification.body, limit);
            out.push_str(&format!("  body: {body}\n"));
        }
    }

    out
}

fn format_inhibitors(inhibitors: &[(u64, String, u32, String)]) -> String {
    // Operational metadata uses the normal bound because no full mode exists here
    let limit = util::default_log_limit();
    let mut out = String::new();
    out.push_str(&format!("inhibitors: {}\n", inhibitors.len()));

    for (id, reason, scope, owner) in inhibitors {
        // Owner and reason both cross the D-Bus trust boundary
        let owner = util::sanitize_log_value(owner, limit);
        let reason = util::sanitize_log_value(reason, limit);
        out.push_str(&format!(
            "- #{id} scope={scope} owner={owner} reason={reason}\n"
        ));
    }

    out
}

#[cfg(test)]
#[path = "tests/notifications.rs"]
mod tests;
