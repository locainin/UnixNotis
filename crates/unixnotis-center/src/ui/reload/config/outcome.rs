//! Reload outcomes and safe failure diagnostics

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use unixnotis_core::{ConfigDiagnostic, ConfigError};
use unixnotis_ui::css::CssReloadReport;

#[derive(Debug)]
pub(in crate::ui) enum ReloadFailure {
    // Parser and path stages stay distinct for stable diagnostics
    Config(ConfigError),
    ThemeBase(String),
    ThemePaths(String),
}

#[derive(Debug)]
pub(in crate::ui) enum ConfigReloadOutcome {
    // Successful reloads retain diagnostics and the matching CSS report
    Applied {
        diagnostics: Vec<ConfigDiagnostic>,
        css: CssReloadReport,
    },
    // Rejections keep the previous live state and expose only the failure category
    Rejected {
        failure: ReloadFailure,
    },
}

impl ReloadFailure {
    pub(super) const fn kind(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::ThemeBase(_) => "theme-base",
            Self::ThemePaths(_) => "theme-paths",
        }
    }

    pub(super) fn safe_fingerprint(&self) -> String {
        // Hash private parser details so distinct failures remain distinguishable without display
        let mut hasher = DefaultHasher::new();
        format!("{self:?}").hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

pub(in crate::ui) fn log_reload_rejection(failure: &ReloadFailure) {
    // Raw parser errors can contain complete config lines, commands, labels, and paths
    tracing::debug!(
        kind = failure.kind(),
        fingerprint = %failure.safe_fingerprint(),
        "config reload rejected"
    );
}
