use unixnotis_core::{IdentityAssurance, Notification, RuleConfig, Urgency};

use crate::store::NotificationStore;

impl NotificationStore {
    pub(super) fn apply_rules(&self, notification: &mut Notification) {
        // Rules are evaluated in config order so later rules can override earlier fields
        for rule in &self.config.rules {
            if !rule_matches(rule, notification) {
                continue;
            }
            apply_rule(rule, notification);
        }
    }
}

fn rule_matches(rule: &RuleConfig, notification: &Notification) -> bool {
    // Every configured filter is ANDed together
    if let Some(app) = rule.app.as_ref() {
        // SECURITY: `Notification::app_name` is sender-controlled protocol metadata
        // `app` rules use daemon-resolved attribution only. Matching the raw claim is
        // intentionally opt-in through `claimed_app`
        let attribution = &notification.attribution;
        if !assurance_allows_app_rule(attribution.assurance) {
            return false;
        }
        let matches_display = contains_ci(&attribution.display_name, app);
        let matches_desktop = contains_ci(&attribution.desktop_id, app);
        if !matches_display && !matches_desktop {
            return false;
        }
    }
    if let Some(claimed_app) = rule.claimed_app.as_ref() {
        if !contains_ci(&notification.app_name, claimed_app) {
            return false;
        }
    }
    if let Some(summary) = rule.summary.as_ref() {
        if !contains_ci(&notification.summary, summary) {
            return false;
        }
    }
    if let Some(body) = rule.body.as_ref() {
        if !contains_ci(&notification.body, body) {
            return false;
        }
    }
    if let Some(category) = rule.category.as_ref() {
        // Missing category means the rule does not match when category filter is requested
        match notification.category.as_ref() {
            Some(value) if contains_ci(value, category) => {}
            _ => return false,
        }
    }
    if let Some(urgency) = rule.urgency {
        if notification.urgency != Urgency::from(urgency) {
            return false;
        }
    }
    true
}

pub(super) const fn assurance_allows_app_rule(assurance: IdentityAssurance) -> bool {
    // Positive enumeration makes new assurance variants fail closed by default
    matches!(
        assurance,
        IdentityAssurance::Authenticated
            | IdentityAssurance::SystemAssociated
            | IdentityAssurance::PortalAssociated
            | IdentityAssurance::UserAssociated
    )
}

fn apply_rule(rule: &RuleConfig, notification: &mut Notification) {
    // Optional fields mutate only when set in the matching rule
    if let Some(no_popup) = rule.no_popup {
        notification.suppress_popup = no_popup;
    }
    if let Some(silent) = rule.silent {
        notification.suppress_sound = silent;
    }
    if let Some(force_urgency) = rule.force_urgency {
        notification.set_urgency(Urgency::from(force_urgency));
    }
    if let Some(expire_timeout_ms) = rule.expire_timeout_ms {
        // Clamp protects against large config values that overflow i32 timeout fields
        let clamped = expire_timeout_ms.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        notification.expire_timeout = clamped;
    }
    if let Some(resident) = rule.resident {
        notification.is_resident = resident;
    }
    if let Some(transient) = rule.transient {
        notification.is_transient = transient;
    }
}

pub(super) fn contains_ci(haystack: &str, needle: &str) -> bool {
    // ASCII case-insensitive substring scan without extra allocations
    if needle.is_empty() {
        return true;
    }
    // Empty needle is handled above, so windows is always safe here
    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return false;
    }
    haystack_bytes
        .windows(needle_bytes.len())
        .any(|window| window.eq_ignore_ascii_case(needle_bytes))
}
