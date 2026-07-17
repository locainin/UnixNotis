//! Center application entrypoint and GTK initialization

#![expect(
    clippy::assigning_clones,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::default_trait_access,
    clippy::future_not_send,
    clippy::items_after_statements,
    clippy::iter_with_drain,
    clippy::literal_string_with_formatting_args,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::needless_pass_by_ref_mut,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::or_fun_call,
    clippy::redundant_locals,
    clippy::ref_option,
    clippy::struct_excessive_bools,
    clippy::struct_field_names,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::useless_let_if_seq,
    reason = "reviewed GTK ownership, pixel conversion, callback, and state-container boundaries require stable signatures and main-thread futures"
)]

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use tracing::info;
use unixnotis_core::Config;

mod control;
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
    // Built-in defaults can run without the installer, so helper scripts are owned here too
    Config::ensure_default_scripts_in(&theme_base).context("ensure default scripts")?;

    info!("center startup checks passed");
    runtime::run_center(config, config_path, theme_paths);
    Ok(())
}
