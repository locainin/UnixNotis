//! Import commit and post-apply validation

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::Config;

use super::super::super::archive::MAX_PRESET_FILE_BYTES;
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
    let config = match load_imported_config(&transaction) {
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

fn load_imported_config(transaction: &super::apply::ImportTransaction) -> Result<Config> {
    // Read through the transaction root so a symlink swap cannot redirect post-apply validation
    let (config_bytes, _mode) = transaction
        .read_file_bounded(Path::new("config.toml"), MAX_PRESET_FILE_BYTES)
        .context("securely read imported config.toml before css-check")?;
    let config_text = std::str::from_utf8(&config_bytes)
        .context("imported config.toml is not valid UTF-8 before css-check")?;
    Config::parse(config_text).context("load imported config.toml before css-check")
}

fn validate_imported_live_config(config_dir: &Path, config: &Config) -> Result<()> {
    // Recheck the live config so `--except config.toml` cannot reuse an outside local theme path
    validate_config_theme_paths_stay_in_root(config_dir, config)?;
    validate_config_command_paths_for_import(config_dir, config)
}
