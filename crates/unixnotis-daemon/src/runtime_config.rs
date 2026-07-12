//! Configuration loading and tracing setup
//!
//! Keeps environment handling and logging setup out of the main control flow

use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;

use super::Args;

pub(super) fn load_config(args: &Args) -> Result<Config> {
    match args.config.as_ref() {
        Some(path) => Config::load_from_path(path).context("read config from path"),
        None => Config::load_default().context("read default config"),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct TracingInitOutcome {
    pub(super) attempted_init: bool,
    pub(super) had_env_warning: bool,
}

pub(super) fn init_tracing(config: &Config) -> TracingInitOutcome {
    let (filter, warning) = match EnvFilter::try_from_default_env() {
        Ok(filter) => (filter, None),
        Err(err) => {
            // Only warn if RUST_LOG was set but invalid; missing env should remain quiet
            let env_warning = if env::var("RUST_LOG").is_ok() {
                Some(format!(
                    "invalid RUST_LOG value: {err}; falling back to config log_level"
                ))
            } else {
                None
            };
            let configured = config
                .general
                .log_level
                .clone()
                .unwrap_or_else(|| "info".to_string());
            let fallback = EnvFilter::try_new(configured.clone()).unwrap_or_else(|err| {
                eprintln!(
                    "unixnotis-daemon: invalid log level '{}': {err}; falling back to info",
                    configured
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

pub(super) async fn ensure_wayland_session(timeout: Duration) -> Result<()> {
    if let Some(display) = detect_wayland_display() {
        apply_wayland_env(&display);
        return Ok(());
    }

    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        if !session_type.eq_ignore_ascii_case("wayland") {
            return Err(anyhow::anyhow!(
                "Wayland session not detected (XDG_SESSION_TYPE={session_type})"
            ));
        }
    }

    if let Some(display) = wait_for_wayland_display(timeout).await {
        apply_wayland_env(&display);
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Wayland session not detected; use --check for config validation"
    ))
}

async fn wait_for_wayland_display(timeout: Duration) -> Option<String> {
    if timeout.is_zero() {
        return None;
    }

    // Bound the retry loop with tokio's timer so a bad loop mutation cannot spin forever
    tokio::time::timeout(timeout, async {
        loop {
            if let Some(display) = detect_wayland_display() {
                break display;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await
    .ok()
}

fn detect_wayland_display() -> Option<String> {
    if let Ok(display) = env::var("WAYLAND_DISPLAY") {
        if wayland_socket_exists(&display) {
            return Some(display);
        }
    }

    // Fallback scan: prefer wayland-0 when WAYLAND_DISPLAY is unset, otherwise accept any socket
    let runtime_dir = env::var("XDG_RUNTIME_DIR").ok()?;
    let entries = fs::read_dir(&runtime_dir).ok()?;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            // Invalid or nameless entries should not abort the whole scan
            continue;
        };
        if !name.starts_with("wayland-") {
            continue;
        }
        #[cfg(unix)]
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                // If file type cannot be inspected, skip the entry to avoid false positives
                continue;
            }
        };
        if !file_type.is_socket() {
            continue;
        }
        #[cfg(not(unix))]
        {
            let _ = entry;
            continue;
        }
        candidates.push(name.to_string());
    }
    choose_wayland_fallback(candidates)
}

fn choose_wayland_fallback(mut candidates: Vec<String>) -> Option<String> {
    candidates.sort();
    candidates.dedup();

    // Prefer the conventional primary socket when present
    if candidates.iter().any(|candidate| candidate == "wayland-0") {
        return Some("wayland-0".to_string());
    }

    match candidates.as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        many => {
            tracing::warn!(
                ?many,
                "multiple Wayland sockets found; set WAYLAND_DISPLAY explicitly"
            );
            None
        }
    }
}

#[cfg(unix)]
fn wayland_socket_exists(display: &str) -> bool {
    let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") else {
        return false;
    };
    let mut path = PathBuf::from(runtime_dir);
    path.push(display);
    match fs::metadata(path) {
        Ok(metadata) => metadata.file_type().is_socket(),
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn wayland_socket_exists(_display: &str) -> bool {
    false
}

fn apply_wayland_env(display: &str) {
    env::set_var("WAYLAND_DISPLAY", display);
    if env::var("XDG_SESSION_TYPE").is_err() {
        env::set_var("XDG_SESSION_TYPE", "wayland");
    }
}

#[cfg(test)]
#[path = "tests/runtime_config/async_wayland.rs"]
mod async_wayland_tests;
#[cfg(test)]
#[path = "tests/runtime_config/config.rs"]
mod config_tests;
#[cfg(test)]
#[path = "tests/runtime_config/detect_wayland.rs"]
mod detect_wayland_tests;
#[cfg(test)]
#[path = "tests/runtime_config/support.rs"]
mod test_support;
#[cfg(test)]
#[path = "tests/runtime_config/tracing.rs"]
mod tracing_tests;
#[cfg(test)]
#[path = "tests/runtime_config/wayland_choice.rs"]
mod wayland_choice_tests;
