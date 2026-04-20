//! Center application entrypoint and GTK initialization.

#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use tracing::info;
use unixnotis_core::Config;

mod dbus;
mod debug;
mod media;
mod runtime;
mod startup;
mod ui;

fn main() -> Result<()> {
    let args = startup::Args::parse();
    let (config, config_path, config_source) =
        startup::load_config(&args).context("load config")?;
    startup::init_tracing(&config);
    let config_source = match config_source {
        startup::ConfigSource::Custom => "custom",
        startup::ConfigSource::Default => "default",
        startup::ConfigSource::Builtin => "builtin",
    };
    info!(config_source, "center configuration loaded");
    if unixnotis_core::util::diagnostic_mode() {
        info!(
            limit = unixnotis_core::util::log_limit(),
            "diagnostic logging enabled (snippets capped; newlines stripped)"
        );
    }

    if !startup::is_wayland_session() {
        return Err(anyhow!(
            "Wayland session not detected; panel UI requires Wayland"
        ));
    }

    let theme_base = Config::config_dir_for_path(&config_path).context("resolve config dir")?;
    let theme_paths = config
        .resolve_theme_paths_from(&theme_base)
        .context("resolve theme paths")?;
    config
        .ensure_theme_files(&theme_paths)
        .context("ensure theme files")?;

    info!("center startup checks passed");
    runtime::run_center(config, config_path, theme_paths);
    Ok(())
}
