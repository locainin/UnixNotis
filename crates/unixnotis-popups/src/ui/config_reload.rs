//! Privacy-safe configuration reload diagnostics for the popup process

use unixnotis_core::ConfigError;

pub(super) fn log_config_rejection(error: &ConfigError) {
    // Parser failures can contain the rejected source line, so only stable fields are emitted
    tracing::warn!(
        kind = config_error_kind(error),
        summary = error.shareable_summary(),
        "failed to reload config"
    );
}

pub(super) fn log_theme_resolution_failure(stage: &'static str) {
    // Path resolution errors may contain account paths that are not needed for runtime logs
    tracing::warn!(%stage, "failed to resolve popup theme inputs");
}

const fn config_error_kind(error: &ConfigError) -> &'static str {
    match error {
        ConfigError::ReadFailed(_) => "read",
        ConfigError::ParseFailed(_) => "parse",
        ConfigError::MissingHome => "missing-home",
    }
}

#[cfg(test)]
#[path = "tests/config_reload.rs"]
mod tests;
