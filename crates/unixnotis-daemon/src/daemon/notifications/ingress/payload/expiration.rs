//! Expiration policy for stored notifications

use std::cmp::Ordering;
use std::time::{Duration, Instant};

use unixnotis_core::{Config, Notification, Urgency};

pub(in crate::daemon::notifications) fn resolve_expiration(
    config: &Config,
    notification: &Notification,
) -> Option<Instant> {
    // Resident notifications stay visible until the sender or user closes them
    if notification.is_resident {
        return None;
    }

    // Zero is an explicit request to disable the expiration timer
    let timeout_ms = match notification.expire_timeout.cmp(&0) {
        Ordering::Equal => return None,
        // Positive values are already bounded at the wire boundary
        Ordering::Greater => notification.expire_timeout as u64,
        // Negative values select the configured urgency default
        Ordering::Less => match notification.urgency {
            Urgency::Critical => config.popups.critical_timeout_ms?,
            _ => config.popups.default_timeout_ms,
        },
    };

    // Avoid allocating a timer deadline when the configured default is disabled
    (timeout_ms != 0).then(|| Instant::now() + Duration::from_millis(timeout_ms))
}
