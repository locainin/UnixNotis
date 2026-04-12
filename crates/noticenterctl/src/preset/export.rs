//! Preset export flow for the live UnixNotis config tree
//!
//! Export reads the active config root, applies explicit exclusions,
//! rejects host-specific escape paths, and writes one shareable bundle file

#[path = "export/checks.rs"]
mod checks;
#[path = "export/prompts.rs"]
mod prompts;
#[cfg(test)]
#[path = "export/tests.rs"]
mod tests;

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use std::path::{Path, PathBuf};
use unixnotis_core::Config;

use self::checks::validate_theme_paths_stay_in_root;
use self::prompts::{
    confirm_export_external_css_refs, prompt_to_fix_host_specific_command_paths,
    rewrite_host_specific_command_paths_if_requested,
};
use super::archive::write_bundle;
use super::command_rules::{validate_config_command_paths_stay_in_root, HostSpecificCommandPath};
use super::config_root::{collect_config_files, override_collected_file_contents};
use super::css_asset_refs::{collect_external_css_asset_refs_from_sources, ExternalCssAssetRef};
use super::manifest::{PresetManifest, PresetManifestFile};
use super::pathing::{
    bundle_name_from_path, format_relative_path, parse_except_paths, resolve_cli_bundle_path,
    validate_preset_bundle_path,
};

#[derive(Debug)]
pub(super) struct ExportSummary {
    // Final bundle file path shown back to the CLI caller
    pub(super) bundle_path: PathBuf,
    // Count of regular files actually stored in the bundle
    pub(super) file_count: usize,
    // Symlinks are reported so the caller can clean them up if needed
    pub(super) skipped_symlinks: Vec<PathBuf>,
    // Non-regular paths are ignored because they are not portable preset content
    pub(super) skipped_non_regular: Vec<PathBuf>,
}

pub(super) fn run_export(output_path: &Path, except: &[String], force: bool) -> Result<()> {
    // Resolve the live config root exactly once for the CLI path
    let config_dir = Config::default_config_dir().context("resolve config directory")?;
    // CLI export accepts a missing extension and can append it after confirmation
    let output_path = resolve_cli_bundle_path(output_path)?;
    let summary = export_preset_from(&config_dir, &output_path, except, force)?;

    println!(
        "preset export ok: {} file(s) -> {}",
        summary.file_count,
        summary.bundle_path.display()
    );
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

pub(super) fn export_preset_from(
    config_dir: &Path,
    output_path: &Path,
    except: &[String],
    force: bool,
) -> Result<ExportSummary> {
    // The shared helper keeps the real prompt path and the test path on the same export logic
    export_preset_from_with_confirm(
        config_dir,
        output_path,
        except,
        force,
        confirm_export_external_css_refs,
        prompt_to_fix_host_specific_command_paths,
    )
}

fn export_preset_from_with_confirm<F, G>(
    config_dir: &Path,
    output_path: &Path,
    except: &[String],
    force: bool,
    confirm_external_css_refs: F,
    prompt_fix_host_specific_command_paths: G,
) -> Result<ExportSummary>
where
    F: FnOnce(&[ExternalCssAssetRef]) -> Result<()>,
    G: FnOnce(&[HostSpecificCommandPath]) -> Result<bool>,
{
    // Tests can inject a fixed answer here so they do not depend on terminal state
    // The orchestrator stays narrow so export-specific checks and prompts can live beside it
    // The user-facing preset extension is part of the public contract
    validate_preset_bundle_path(output_path)?;
    if !config_dir.exists() {
        return Err(anyhow!(
            "config directory not found: {}",
            config_dir.display()
        ));
    }
    if !config_dir.is_dir() {
        return Err(anyhow!(
            "config path is not a directory: {}",
            config_dir.display()
        ));
    }
    if output_path.exists() && !force {
        return Err(anyhow!(
            "preset bundle already exists (use --force to overwrite): {}",
            output_path.display()
        ));
    }

    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Err(anyhow!(
            "preset export requires config.toml in {}",
            config_dir.display()
        ));
    }

    // Loading the live config up front catches broken bundles before export starts
    let mut config =
        Config::load_from_path(&config_path).context("load config.toml for preset export")?;
    let theme_paths = config
        .resolve_theme_paths_from(config_dir)
        .context("resolve active theme paths for preset export")?;
    // Active theme targets must stay inside the config root so the bundle is truly portable
    validate_theme_paths_stay_in_root(
        config_dir,
        &[
            ("base_css", &theme_paths.base_css),
            ("panel_css", &theme_paths.panel_css),
            ("popup_css", &theme_paths.popup_css),
            ("widgets_css", &theme_paths.widgets_css),
            ("media_css", &theme_paths.media_css),
        ],
    )?;
    // Shared presets should not ship explicit command paths that depend on outside host files
    validate_config_command_paths_stay_in_root(
        config_dir,
        &config,
        "preset export requires explicit command paths to stay under the config root",
    )?;
    // Absolute command paths under the config root still leak the local machine layout into the preset
    let leaked_command_paths = rewrite_host_specific_command_paths_if_requested(
        config_dir,
        &mut config,
        prompt_fix_host_specific_command_paths,
    )?;

    let exclusions = parse_except_paths(except)?;
    if exclusions
        .iter()
        .any(|path| path == Path::new("config.toml"))
    {
        // Import depends on config.toml to describe the shared setup
        return Err(anyhow!(
            "preset export cannot exclude config.toml because the bundle would not be importable"
        ));
    }

    // File collection walks the whole config tree and filters backup dirs and excluded paths
    let mut collected = collect_config_files(config_dir, Some(output_path), &exclusions)?;
    if !collected
        .files
        .iter()
        .any(|file| file.relative_path == Path::new("config.toml"))
    {
        return Err(anyhow!(
            "preset export did not capture config.toml after applying exclusions"
        ));
    }
    if collected.files.is_empty() {
        return Err(anyhow!("preset export found no files to bundle"));
    }

    if !leaked_command_paths.is_empty() {
        // Only the bundled config is rewritten so the live config tree stays untouched
        let config_bytes = toml::to_string_pretty(&config)
            .context("encode fixed config.toml for preset export")?
            .into_bytes();
        override_collected_file_contents(&mut collected, Path::new("config.toml"), config_bytes)?;
        eprintln!(
            "preset export note: rewrote {} host-specific command path(s) in the bundled config.toml",
            leaked_command_paths.len()
        );
    }

    // Warn before writing the bundle when shared CSS depends on outside assets
    let external_css_refs =
        collect_external_css_asset_refs_from_sources(config_dir, &collected.files)?;
    confirm_external_css_refs(&external_css_refs)?;

    let manifest_files = collected
        .files
        .iter()
        .map(|file| PresetManifestFile {
            // Manifest stores slash-separated relative paths for stable cross-platform output
            path: format_relative_path(&file.relative_path),
            size: file.size,
        })
        .collect::<Vec<_>>();
    // Manifest metadata is lightweight and lets inspect work without unpacking to disk
    let manifest = PresetManifest::new(
        bundle_name_from_path(output_path)?,
        Local::now().to_rfc3339(),
        env!("CARGO_PKG_VERSION").to_string(),
        manifest_files,
    );
    write_bundle(output_path, &manifest, &collected).context("write preset bundle")?;

    Ok(ExportSummary {
        bundle_path: output_path.to_path_buf(),
        file_count: collected.files.len(),
        skipped_symlinks: collected.skipped_symlinks,
        skipped_non_regular: collected.skipped_non_regular,
    })
}
