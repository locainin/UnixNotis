//! Stable service-manager identities shared by installers and diagnostics

use std::str::FromStr;

use serde::Serialize;

use super::ServiceManagerPathError;

/// Service managers supported by the `UnixNotis` installer and diagnostics
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceManagerKind {
    /// Systemd user service
    Systemd,
    /// Dinit user service
    Dinit,
    /// Runit user service directory
    Runit,
    /// S6-rc user service database
    S6,
}

impl ServiceManagerKind {
    /// Every supported manager in stable probe order
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Systemd, Self::Dinit, Self::Runit, Self::S6]
    }

    /// Parse environment and CLI aliases without applying an implicit default
    ///
    /// # Errors
    ///
    /// Returns an error when the value does not name a supported manager
    pub fn parse(raw: &str) -> Result<Self, ServiceManagerPathError> {
        raw.parse()
    }

    /// Stable user-facing backend label
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Systemd => "systemd",
            Self::Dinit => "dinit",
            Self::Runit => "runit",
            Self::S6 => "s6-rc",
        }
    }
}

impl FromStr for ServiceManagerKind {
    type Err = ServiceManagerPathError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim() {
            "systemd" | "systemd-user" => Ok(Self::Systemd),
            "dinit" | "dinit-user" => Ok(Self::Dinit),
            "runit" | "runit-user" => Ok(Self::Runit),
            "s6" | "s6-user" => Ok(Self::S6),
            other => Err(ServiceManagerPathError::Unsupported(other.to_string())),
        }
    }
}
