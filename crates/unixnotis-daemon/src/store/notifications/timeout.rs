//! Resolve popup visibility and daemon lifetime clocks at commit time

use std::time::Duration;

use unixnotis_core::{Config, Notification, Urgency};

/// Sanitized timeout decisions shared by the daemon scheduler and popup view
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolvedTimeoutPolicy {
    /// Zero keeps the banner visible until an explicit close or replacement
    pub(super) popup_hide_after_ms: u64,
    /// None keeps the active record available for panel actions indefinitely
    pub(super) active_close_after: Option<Duration>,
}

/// Resolve both clocks after rule mutations and before the generation is committed
pub(super) fn resolve_timeout_policy(
    config: &Config,
    notification: &Notification,
) -> ResolvedTimeoutPolicy {
    let configured_popup_ms = match notification.urgency {
        Urgency::Critical => config.popups.critical_timeout_ms.unwrap_or(0),
        _ => config.popups.default_timeout_ms,
    };

    match notification.expire_timeout {
        // A zero protocol timeout disables both automatic clocks
        0 => ResolvedTimeoutPolicy {
            popup_hide_after_ms: 0,
            active_close_after: None,
        },
        // Positive protocol values close normal notifications regardless of resident state
        timeout if timeout > 0 => {
            let timeout_ms = timeout as u64;
            ResolvedTimeoutPolicy {
                popup_hide_after_ms: timeout_ms,
                // `resident` controls post-action dismissal. It does not override the
                // notification's explicit expiration timeout
                active_close_after: (notification.urgency != Urgency::Critical)
                    .then(|| Duration::from_millis(timeout_ms)),
            }
        }
        // The default protocol value uses UnixNotis display policy
        _ => {
            // Critical popup visibility and active-notification lifetime are separate
            // Critical alerts may leave the screen, but stay active until explicitly closed
            let active_close_after = if notification.urgency != Urgency::Critical
                && notification.is_transient
                && configured_popup_ms > 0
            {
                Some(Duration::from_millis(configured_popup_ms))
            } else {
                None
            };

            ResolvedTimeoutPolicy {
                popup_hide_after_ms: configured_popup_ms,
                active_close_after,
            }
        }
    }
}
