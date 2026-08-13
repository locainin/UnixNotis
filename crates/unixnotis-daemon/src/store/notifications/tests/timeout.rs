use std::time::Duration;

use super::super::timeout::resolve_timeout_policy;
use super::support::make_notification;
use unixnotis_core::{Config, Urgency};

#[test]
fn zero_protocol_timeout_disables_both_clocks() {
    let config = Config::default();
    let mut notification = make_notification("never");
    notification.expire_timeout = 0;

    assert_eq!(
        resolve_timeout_policy(&config, &notification),
        super::super::timeout::ResolvedTimeoutPolicy {
            popup_hide_after_ms: 0,
            active_close_after: None,
        }
    );
}

#[test]
fn positive_protocol_timeout_closes_nonresident_notifications() {
    let config = Config::default();
    let mut notification = make_notification("bounded");
    notification.expire_timeout = 30_000;

    assert_eq!(
        resolve_timeout_policy(&config, &notification),
        super::super::timeout::ResolvedTimeoutPolicy {
            popup_hide_after_ms: 30_000,
            active_close_after: Some(Duration::from_secs(30)),
        }
    );
}

#[test]
fn positive_protocol_timeout_closes_transient_notifications() {
    let config = Config::default();
    let mut notification = make_notification("transient-bounded");
    notification.expire_timeout = 30_000;
    notification.is_transient = true;

    assert_eq!(
        resolve_timeout_policy(&config, &notification),
        super::super::timeout::ResolvedTimeoutPolicy {
            popup_hide_after_ms: 30_000,
            active_close_after: Some(Duration::from_secs(30)),
        }
    );
}

#[test]
fn resident_positive_timeout_still_expires_the_active_notification() {
    let config = Config::default();
    let mut notification = make_notification("resident");
    notification.expire_timeout = 30_000;
    notification.is_resident = true;

    let policy = resolve_timeout_policy(&config, &notification);
    assert_eq!(policy.popup_hide_after_ms, 30_000);
    assert_eq!(policy.active_close_after, Some(Duration::from_secs(30)));
}

#[test]
fn critical_positive_timeout_hides_popup_without_expiring_active_notification() {
    let config = Config::default();
    let mut notification = make_notification("critical bounded popup");
    notification.expire_timeout = 30_000;
    notification.urgency = Urgency::Critical;

    let policy = resolve_timeout_policy(&config, &notification);

    assert_eq!(policy.popup_hide_after_ms, 30_000);
    assert_eq!(policy.active_close_after, None);
}

#[test]
fn default_normal_timeout_hides_but_keeps_active_record() {
    let config = Config::default();
    let mut notification = make_notification("normal");
    notification.expire_timeout = -1;

    let policy = resolve_timeout_policy(&config, &notification);
    assert_eq!(policy.popup_hide_after_ms, config.popups.default_timeout_ms);
    assert_eq!(policy.active_close_after, None);
}

#[test]
fn default_critical_without_timeout_stays_visible() {
    let config = Config::default();
    let mut notification = make_notification("critical");
    notification.expire_timeout = -1;
    notification.urgency = Urgency::Critical;

    let policy = resolve_timeout_policy(&config, &notification);
    assert_eq!(policy.popup_hide_after_ms, 0);
    assert_eq!(policy.active_close_after, None);
}

#[test]
fn transient_default_timeout_closes_without_history_by_default() {
    let config = Config::default();
    let mut notification = make_notification("transient");
    notification.expire_timeout = -1;
    notification.is_transient = true;

    let policy = resolve_timeout_policy(&config, &notification);
    assert_eq!(policy.popup_hide_after_ms, config.popups.default_timeout_ms);
    assert_eq!(
        policy.active_close_after,
        Some(Duration::from_millis(config.popups.default_timeout_ms))
    );
}

#[test]
fn resident_transient_default_timeout_uses_time_policy_independent_of_actions() {
    let config = Config::default();
    let mut notification = make_notification("resident transient");
    notification.expire_timeout = -1;
    notification.is_transient = true;
    notification.is_resident = true;

    let policy = resolve_timeout_policy(&config, &notification);

    assert_eq!(policy.popup_hide_after_ms, config.popups.default_timeout_ms);
    assert_eq!(
        policy.active_close_after,
        Some(Duration::from_millis(config.popups.default_timeout_ms))
    );
}
