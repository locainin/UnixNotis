//! Output formatting helpers for noticenterctl.

use unixnotis_core::{util, NotificationView};

pub(crate) fn print_notifications(label: &str, notifications: &[NotificationView], full: bool) {
    // Respect the diagnostic mode guard so secrets are not printed unintentionally.
    let limit = if full {
        util::diagnostic_log_limit()
    } else {
        util::default_log_limit()
    };

    // The header keeps list output consistent across commands.
    println!("{} notifications: {}", label, notifications.len());
    for notification in notifications {
        // Log output is sanitized to avoid terminal control characters and long blobs.
        let summary = util::sanitize_log_value(&notification.summary, limit);
        // Each line is stable and script-friendly for downstream tooling.
        println!(
            "- #{id} [{app}] {summary}",
            id = notification.id,
            app = notification.app_name,
            summary = summary
        );
    }
}
