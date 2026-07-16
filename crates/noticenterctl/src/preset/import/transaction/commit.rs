//! Import commit and post-apply validation

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::Config;

use super::super::review::checks::{
    validate_config_command_paths_for_import, validate_config_theme_paths_stay_in_root,
};
use super::apply::{apply_import_plan, finalize_import_transaction, rollback_import_transaction};
use super::plan::ImportPlan;

pub(in crate::preset) fn commit_import_plan(
    config_dir: &Path,
    plan: &ImportPlan,
    run_css_check: impl FnOnce() -> Result<()>,
) -> Result<(Option<PathBuf>, Result<()>)> {
    // Apply first, then keep the transaction open until the post-import checks finish
    let transaction = apply_import_plan(config_dir, plan)?;
    let config = match load_imported_config(config_dir) {
        Ok(config) => config,
        Err(err) => {
            rollback_import_transaction(transaction)?;
            return Err(err);
        }
    };

    if let Err(err) = validate_imported_live_config(config_dir, &config) {
        rollback_import_transaction(transaction)?;
        return Err(err);
    }

    // Imported presets should be checked right away so broken shared CSS is obvious
    println!("preset import check: running css-check on imported theme files");
    let css_check_result = run_css_check();
    let backup_dir = finalize_import_transaction(transaction)?;
    Ok((backup_dir, css_check_result))
}

fn load_imported_config(config_dir: &Path) -> Result<Config> {
    // Reload the active config after import so css-check validates the setup that was just applied
    let config_path = config_dir.join("config.toml");
    Config::load_from_path(&config_path).context("load imported config.toml before css-check")
}

fn validate_imported_live_config(config_dir: &Path, config: &Config) -> Result<()> {
    // Recheck the live config so `--except config.toml` cannot reuse an outside local theme path
    validate_config_theme_paths_stay_in_root(config_dir, config)?;
    validate_config_command_paths_for_import(config_dir, config)
}
