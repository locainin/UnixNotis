//! User-facing preset export command

use std::path::Path;

use anyhow::{Context, Result};
use unixnotis_core::Config;

use super::flow::export_preset_from;
use crate::preset::pathing::resolve_cli_bundle_path;

pub(in crate::preset) fn run_export(
    output_path: &Path,
    except: &[String],
    force: bool,
) -> Result<()> {
    // Resolve the live config root once so every later path shares one anchor
    let config_dir = Config::default_config_dir().context("resolve config directory")?;
    // CLI export accepts a missing extension and can append it after confirmation
    let output_path = resolve_cli_bundle_path(output_path)?;
    let summary = export_preset_from(&config_dir, &output_path, except, force)?;

    println!("{}", export_success_line(&summary));
    if !summary.skipped_symlinks.is_empty() {
        eprintln!(
            "preset export warning: skipped {} symlink path(s)",
            summary.skipped_symlinks.len()
        );
    }
    if !summary.skipped_non_regular.is_empty() {
        eprintln!(
            "preset export warning: skipped {} non-regular path(s)",
            summary.skipped_non_regular.len()
        );
    }
    Ok(())
}

fn export_success_line(summary: &super::model::ExportSummary) -> String {
    // Keep the stable CLI summary independent from terminal output side effects
    format!(
        "preset export ok: {} file(s) -> {}",
        summary.file_count,
        summary.bundle_path.display()
    )
}

#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
