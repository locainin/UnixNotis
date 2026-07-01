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
        // Environment-based selection treats an empty value like an unset value
        // so old shell exports do not force an installer failure
        match raw.trim() {
            "" | "systemd" | "systemd-user" => Ok(Self::Systemd),
            "dinit" | "dinit-user" => Ok(Self::Dinit),
            "runit" | "runit-user" => Ok(Self::Runit),
            "s6" | "s6-user" => Ok(Self::S6),
            other => Err(anyhow!("unsupported service manager '{other}'")),
        }
    }

    pub fn parse_explicit(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();

        // CLI values are intentional user input. Empty values should be called
        // out instead of being treated like the default backend
        if trimmed.is_empty() {
            return Err(anyhow!("unsupported service manager ''"));
        }

        // Reuse the shared backend name list after the explicit-value guard
        Self::parse(trimmed)
    }
}
