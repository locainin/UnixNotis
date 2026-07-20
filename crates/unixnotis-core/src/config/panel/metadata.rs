//! Configurable notification metadata text

use serde::{Deserialize, Serialize};

/// Text and compact templates used by optional notification metadata lanes
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct NotificationMetadataConfig {
    pub critical_label: String,
    pub low_label: String,
    pub normal_label: String,
    pub relative_now: String,
    /// Minute template where `{value}` is replaced with the elapsed count
    pub relative_minutes: String,
    /// Hour template where `{value}` is replaced with the elapsed count
    pub relative_hours: String,
    /// Day template where `{value}` is replaced with the elapsed count
    pub relative_days: String,
    pub transient_label: String,
    pub live_label: String,
    pub history_label: String,
    /// Singular template where `{count}` is replaced with one
    pub action_count_one: String,
    /// Plural template where `{count}` is replaced with the visible action count
    pub action_count_many: String,
}

impl Default for NotificationMetadataConfig {
    fn default() -> Self {
        Self {
            critical_label: "ALERT".to_string(),
            low_label: "LOW".to_string(),
            normal_label: "NOTICE".to_string(),
            relative_now: "now".to_string(),
            relative_minutes: "{value}m".to_string(),
            relative_hours: "{value}h".to_string(),
            relative_days: "{value}d".to_string(),
            transient_label: "TRANSIENT".to_string(),
            live_label: "LIVE".to_string(),
            history_label: "HISTORY".to_string(),
            action_count_one: "{count} ACTION".to_string(),
            action_count_many: "{count} ACTIONS".to_string(),
        }
    }
}
