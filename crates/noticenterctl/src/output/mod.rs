//! Output formatting helpers for noticenterctl

mod gate;

use unixnotis_core::{util, NotificationView};

pub use gate::{allow_full_output, warn_full_requires_diagnostic};

pub fn print_notifications(label: &str, notifications: &[NotificationView], full: bool) {
    // One place for CLI output
    print!("{}", format_notifications(label, notifications, full));
}

pub fn print_inhibitors(inhibitors: &[(u64, String, u32, String)]) {
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
#[path = "tests/cases.rs"]
mod tests;
