//! Tracing initialization and fallback policy

use std::env;

use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;

#[derive(Debug, PartialEq, Eq)]
pub struct TracingInitOutcome {
    pub(super) attempted_init: bool,
    pub(super) had_env_warning: bool,
}

pub fn init_tracing(config: &Config) -> TracingInitOutcome {
    let (filter, warning) = match EnvFilter::try_from_default_env() {
        Ok(filter) => (filter, None),
        Err(err) => {
            // Invalid explicit filters warn while a missing environment value stays quiet
            let env_warning = env::var("RUST_LOG").is_ok().then(|| {
                format!("invalid RUST_LOG value: {err}; falling back to config log_level")
            });
            let configured = config
                .general
                .log_level
                .clone()
                .unwrap_or_else(|| "info".to_string());
            let fallback = EnvFilter::try_new(configured.clone()).unwrap_or_else(|err| {
                eprintln!(
                    "unixnotis-daemon: invalid log level '{configured}': {err}; falling back to info"
                );
                EnvFilter::new("info")
            });
            (fallback, env_warning)
        }
    };
    if let Err(err) = tracing_subscriber::fmt().with_env_filter(filter).try_init() {
        eprintln!("unixnotis-daemon: tracing already initialized or unavailable: {err}");
    }
    let had_env_warning = warning.is_some();
    if let Some(message) = warning {
        tracing::warn!("{message}");
    }
    TracingInitOutcome {
        attempted_init: true,
        had_env_warning,
    }
}
