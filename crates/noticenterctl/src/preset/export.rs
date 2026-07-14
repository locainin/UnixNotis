//! Preset export flow for the live `UnixNotis` config tree
//!
//! Export reads the active config root, applies explicit exclusions,
//! rejects host-specific escape paths, and writes one shareable bundle file

mod checks;
mod prompts;
mod script_dependencies;
#[cfg(test)]
#[path = "export/tests/script_dependencies.rs"]
mod script_dependency_tests;
#[cfg(test)]
#[path = "export/tests/cases.rs"]
mod tests;

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use std::path::{Path, PathBuf};
use toml::Value;
use unixnotis_core::{validate_icon_asset_reference, Config};

use self::checks::{validate_theme_paths_stay_in_root, HostSpecificScriptLeak};
use self::prompts::{
    confirm_export_external_css_refs, prompt_to_fix_host_specific_command_paths,
    prompt_to_fix_host_specific_css_asset_refs, prompt_to_fix_host_specific_script_paths,
    rewrite_host_specific_command_paths_if_requested,
    rewrite_host_specific_css_asset_refs_if_requested,
    rewrite_host_specific_script_paths_if_requested,
};
use self::script_dependencies::collect_script_dependency_closure;
use super::archive::write_bundle;
use super::command_rules::{
    collect_command_references_from_config, resolve_command_path_token,
    validate_config_command_paths_stay_in_root, HostSpecificCommandPath,
};
use super::config_root::{collect_selected_config_files, override_collected_file_contents};
use super::css_asset_refs::{
    collect_external_css_asset_refs_from_collected, collect_local_css_asset_paths_from_paths,
    ExternalCssAssetRef, HostSpecificCssAssetRef,
};
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
        ExportConfirmers {
            confirm_external_css_refs: confirm_export_external_css_refs,
            prompt_fix_host_specific_command_paths: prompt_to_fix_host_specific_command_paths,
            prompt_fix_host_specific_css_asset_refs: prompt_to_fix_host_specific_css_asset_refs,
            prompt_fix_host_specific_script_paths: prompt_to_fix_host_specific_script_paths,
        },
    )
}

type ConfirmExternalCssRefsFn = fn(&[ExternalCssAssetRef]) -> Result<()>;
type PromptFixCommandPathsFn = fn(&[HostSpecificCommandPath]) -> Result<bool>;
type PromptFixCssAssetRefsFn = fn(&[HostSpecificCssAssetRef]) -> Result<bool>;
type PromptFixScriptPathsFn = fn(&[HostSpecificScriptLeak]) -> Result<bool>;

struct ExportConfirmers {
    // Guard for CSS refs that leave the config root
    confirm_external_css_refs: ConfirmExternalCssRefsFn,
    // Prompt hook for command path rewrite flow
    prompt_fix_host_specific_command_paths: PromptFixCommandPathsFn,
    // Prompt hook for CSS asset rewrite flow
    prompt_fix_host_specific_css_asset_refs: PromptFixCssAssetRefsFn,
    // Prompt hook for script text rewrite flow
    prompt_fix_host_specific_script_paths: PromptFixScriptPathsFn,
}

fn export_preset_from_with_confirm(
    config_dir: &Path,
    output_path: &Path,
    except: &[String],
    force: bool,
    confirmers: ExportConfirmers,
) -> Result<ExportSummary> {
    // Tests inject fixed handlers here so behavior is deterministic
    // Keep preset extension checks close to the entry point
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
        confirmers.prompt_fix_host_specific_command_paths,
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

    // Build a dependency closure instead of copying unrelated files from the config tree
    let theme_files = [
        theme_paths.base_css,
        theme_paths.panel_css,
        theme_paths.popup_css,
        theme_paths.widgets_css,
        theme_paths.media_css,
    ];
    let mut selected_paths = vec![PathBuf::from("config.toml")];
    let existing_theme_files = theme_files
        .iter()
        .filter(|path| path.is_file())
        .cloned()
        .collect::<Vec<_>>();
    for theme_file in &existing_theme_files {
        selected_paths.push(
            theme_file
                .strip_prefix(config_dir)
                .context("make active theme path relative to config root")?
                .to_path_buf(),
        );
    }
    selected_paths.extend(collect_local_css_asset_paths_from_paths(
        config_dir,
        &existing_theme_files,
    )?);
    let command_script_paths = collect_command_references_from_config(&config)
        .into_iter()
        .filter_map(|reference| resolve_command_path_token(config_dir, &reference.command))
        .filter(|path| path.is_file())
        .filter_map(|path| path.strip_prefix(config_dir).ok().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    // Direct config commands can source small helper libraries that are just as required as the entry script
    // Resolve that closure before collection so a preset remains usable on a clean installation
    selected_paths.extend(collect_script_dependency_closure(
        config_dir,
        &command_script_paths,
    )?);
    selected_paths.extend(collect_existing_icon_assets(&config_path, config_dir)?);
    selected_paths.sort();
    selected_paths.dedup();

    let mut collected =
        collect_selected_config_files(config_dir, &selected_paths, Some(output_path), &exclusions)?;
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

    let leaked_script_paths = rewrite_host_specific_script_paths_if_requested(
        config_dir,
        &mut collected,
        confirmers.prompt_fix_host_specific_script_paths,
    )?;
    if !leaked_script_paths.is_empty() {
        // Report bundled script rewrites for audit visibility
        eprintln!(
            "preset export note: rewrote {} host-specific script path reference(s) in bundled script files",
            leaked_script_paths.len()
        );
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

    let leaked_css_asset_refs = rewrite_host_specific_css_asset_refs_if_requested(
        config_dir,
        &mut collected,
        confirmers.prompt_fix_host_specific_css_asset_refs,
    )?;
    if !leaked_css_asset_refs.is_empty() {
        eprintln!(
            "preset export note: rewrote {} host-specific CSS asset reference(s) in the bundled stylesheet(s)",
            leaked_css_asset_refs.len()
        );
    }

    // Warn before writing the bundle when shared CSS depends on outside assets
    let external_css_refs =
        collect_external_css_asset_refs_from_collected(config_dir, &collected.files)?;
    (confirmers.confirm_external_css_refs)(&external_css_refs)?;

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

fn collect_existing_icon_assets(config_path: &Path, config_dir: &Path) -> Result<Vec<PathBuf>> {
    let config_text = std::fs::read_to_string(config_path)
        .with_context(|| format!("read config file {}", config_path.display()))?;
    let value: Value = toml::from_str(&config_text).context("parse config.toml icon assets")?;
    let mut raw_assets = Vec::new();
    collect_icon_asset_values(&value, &mut raw_assets);

    let mut paths = Vec::new();
    for asset in raw_assets {
        validate_icon_asset_reference(&asset)
            .with_context(|| format!("validate configured icon asset {asset}"))?;
        let relative = PathBuf::from(asset);
        if config_dir.join(&relative).is_file() {
            paths.push(relative);
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn collect_icon_asset_values(value: &Value, assets: &mut Vec<String>) {
    match value {
        Value::Table(table) => {
            for (key, child) in table {
                if key == "icon_asset" {
                    if let Some(asset) = child.as_str().filter(|asset| !asset.trim().is_empty()) {
                        assets.push(asset.trim().to_string());
                    }
                }
                collect_icon_asset_values(child, assets);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_icon_asset_values(child, assets);
            }
        }
        _ => {}
    }
}
