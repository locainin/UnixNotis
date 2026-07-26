//! Daemon entrypoint and service bootstrap

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_let_else,
    clippy::needless_continue,
    clippy::needless_pass_by_ref_mut,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::ref_option,
    clippy::significant_drop_tightening,
    clippy::struct_excessive_bools,
    clippy::struct_field_names,
    clippy::trivially_copy_pass_by_ref,
    clippy::unnecessary_wraps,
    clippy::unused_async,
    reason = "reviewed D-Bus trait signatures, lock lifetimes, protocol integer widths, and private-module visibility preserve daemon compatibility"
)]

use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

mod child_process;
mod cli;
mod daemon;
mod dnd_expiration;
mod expire;
mod runtime;
mod sound;
mod startup;
mod store;
mod system_tools;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
mod trial_mode;

use crate::cli::Args;
use crate::startup::{ensure_wayland_session, init_tracing, load_config};
use unixnotis_core::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = load_config(&args).context("load config")?;

    init_tracing(&config);
    let config_source = if args.config.is_some() {
        "custom"
    } else {
        match Config::default_config_path() {
            Ok(path) if path.exists() => "default",
            _ => "builtin",
        }
    };
    info!(config_source, "configuration loaded");
    if unixnotis_core::util::diagnostic_mode() {
        info!(
            limit = unixnotis_core::util::log_limit(),
            "diagnostic logging enabled (snippets capped; newlines stripped)"
        );
    }

    if args.check {
        info!("configuration loaded successfully");
        return Ok(());
    }

    ensure_wayland_session(Duration::from_secs(20))
        .await
        .context("wait for Wayland session")?;
    Box::pin(runtime::run(&args, config)).await
}
