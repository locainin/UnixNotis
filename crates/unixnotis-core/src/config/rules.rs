//! Notification rule configuration and validation.

use serde::de::{self, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::Urgency;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RuleUrgency {
    Low,
    Normal,
    Critical,
}

impl RuleUrgency {
    /// Strictly validate numeric urgency values during deserialization.
    fn from_u8(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Normal),
            2 => Ok(Self::Critical),
            _ => Err("urgency must be 0 (low), 1 (normal), or 2 (critical)".to_string()),
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::Critical => 2,
        }
    }
}

impl From<RuleUrgency> for Urgency {
    fn from(value: RuleUrgency) -> Self {
        match value {
            RuleUrgency::Low => Self::Low,
            RuleUrgency::Normal => Self::Normal,
            RuleUrgency::Critical => Self::Critical,
        }
    }
}

impl Serialize for RuleUrgency {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Persist numeric urgency values for stable config output.
        serializer.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for RuleUrgency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RuleUrgencyVisitor;

        impl<'de> Visitor<'de> for RuleUrgencyVisitor {
            type Value = RuleUrgency;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an urgency value (0=low, 1=normal, 2=critical)")
            }

            fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                RuleUrgency::from_u8(value).map_err(E::custom)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = u8::try_from(value).map_err(|_| {
                    E::custom("urgency must be 0 (low), 1 (normal), or 2 (critical)")
                })?;
                RuleUrgency::from_u8(value).map_err(E::custom)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value < 0 || value > u8::MAX as i64 {
                    return Err(E::custom(
                        "urgency must be 0 (low), 1 (normal), or 2 (critical)",
                    ));
                }
                RuleUrgency::from_u8(value as u8).map_err(E::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "0" | "low" => Ok(RuleUrgency::Low),
                    "1" | "normal" => Ok(RuleUrgency::Normal),
                    "2" | "critical" => Ok(RuleUrgency::Critical),
                    _ => Err(E::custom(
                        "urgency must be 0 (low), 1 (normal), or 2 (critical)",
                    )),
                }
            }
        }

        deserializer.deserialize_any(RuleUrgencyVisitor)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RuleConfig {
    /// Optional rule name for logging or debugging.
    pub name: Option<String>,
    /// Match against the notification app name (case-insensitive substring).
    pub app: Option<String>,
    /// Match against the notification summary (case-insensitive substring).
    pub summary: Option<String>,
    /// Match against the notification body (case-insensitive substring).
    pub body: Option<String>,
    /// Match against the notification category hint (case-insensitive substring).
    pub category: Option<String>,
    /// Match against urgency (0=low, 1=normal, 2=critical).
    pub urgency: Option<RuleUrgency>,
    /// Suppress popups when true.
    pub no_popup: Option<bool>,
    /// Suppress sound when true.
    pub silent: Option<bool>,
    /// Force urgency when set (0=low, 1=normal, 2=critical).
    pub force_urgency: Option<RuleUrgency>,
    /// Override expire timeout in milliseconds (-1 for default, 0 for no expire).
    pub expire_timeout_ms: Option<i64>,
    /// Override resident flag when set.
    pub resident: Option<bool>,
    /// Override transient flag when set.
    pub transient: Option<bool>,
}
