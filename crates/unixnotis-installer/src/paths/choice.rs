//! Service-manager selection values

use anyhow::{anyhow, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceManagerChoice {
    // Stable default backend for normal Linux desktop sessions
    Systemd,
    // Experimental user dinit backend with artifact-backed enablement
    Dinit,
    // Experimental user runit backend for runsvdir-managed service trees
    Runit,
    // Experimental local-user s6 backend with an installer-compiled s6-rc database
    S6,
}

impl ServiceManagerChoice {
    pub(super) fn all() -> [Self; 4] {
        // Conflict scans need to inspect every supported backend, including experimental ones
        [Self::Systemd, Self::Dinit, Self::Runit, Self::S6]
    }

    pub fn parse(raw: &str) -> Result<Self> {
        // Accept both short names and explicit user-backend names for CLI/env symmetry
        match raw.trim() {
            "" | "systemd" | "systemd-user" => Ok(Self::Systemd),
            "dinit" | "dinit-user" => Ok(Self::Dinit),
            "runit" | "runit-user" => Ok(Self::Runit),
            "s6" | "s6-user" => Ok(Self::S6),
            other => Err(anyhow!("unsupported service manager '{other}'")),
        }
    }
}
