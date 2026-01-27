//! Configuration loading and tracing setup.
//!
//! Keeps environment handling and logging setup out of the main control flow.

use std::env;
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

pub(super) fn init_tracing(config: &Config) {
    let (filter, warning) = match EnvFilter::try_from_default_env() {
        Ok(filter) => (filter, None),
        Err(err) => {
            // Only warn if RUST_LOG was set but invalid; missing env should remain quiet.
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
    tracing_subscriber::fmt().with_env_filter(filter).init();
    if let Some(message) = warning {
        tracing::warn!("{message}");
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

    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(display) = detect_wayland_display() {
            apply_wayland_env(&display);
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Err(anyhow::anyhow!(
        "Wayland session not detected; use --check for config validation"
    ))
}

fn detect_wayland_display() -> Option<String> {
    if let Ok(display) = env::var("WAYLAND_DISPLAY") {
        if wayland_socket_exists(&display) {
            return Some(display);
        }
    }

    // Fallback scan: prefer wayland-0 when WAYLAND_DISPLAY is unset, otherwise accept any socket.
    let runtime_dir = env::var("XDG_RUNTIME_DIR").ok()?;
    let entries = fs::read_dir(&runtime_dir).ok()?;
    let mut fallback = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_string_lossy().to_string();
        if !name.starts_with("wayland-") {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                // If file type cannot be inspected, skip the entry to avoid false positives.
                continue;
            }
        };
        if !file_type.is_socket() {
            continue;
        }
        if name == "wayland-0" {
            return Some(name);
        }
        fallback.get_or_insert(name);
    }
    fallback
}

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

fn apply_wayland_env(display: &str) {
    env::set_var("WAYLAND_DISPLAY", display);
    if env::var("XDG_SESSION_TYPE").is_err() {
        env::set_var("XDG_SESSION_TYPE", "wayland");
    }
}
