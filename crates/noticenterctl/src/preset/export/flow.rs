//! Secure preset selection, rewrite, and archive flow

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use std::path::{Path, PathBuf};

use super::assets::collect_existing_icon_assets;
use super::checks::validate_theme_paths_stay_in_root;
use super::model::{ExportConfirmers, ExportSummary};
use super::prompts::{
    confirm_export_external_css_refs, prompt_to_fix_host_specific_command_paths,
    prompt_to_fix_host_specific_css_asset_refs, prompt_to_fix_host_specific_script_paths,
    rewrite_host_specific_command_paths_if_requested,
    rewrite_host_specific_css_asset_refs_if_requested,
    rewrite_host_specific_script_paths_if_requested,
};
use super::script_dependencies::collect_script_dependency_closure_from_root;
use super::source::ExportSourceSnapshot;
use crate::preset::archive::write_bundle;
use crate::preset::command_rules::{
    collect_command_references_from_config, resolve_command_path_token,
    validate_config_command_paths_stay_in_root,
};
use crate::preset::config_root::{
    collect_selected_config_files_from_root, override_collected_file_contents,
};
use crate::preset::css_asset_refs::{
    collect_external_css_asset_refs_from_collected, collect_local_css_asset_paths_from_captures,
};
use crate::preset::manifest::{PresetManifest, PresetManifestFile};
use crate::preset::pathing::{
    bundle_name_from_path, format_relative_path, parse_except_paths, validate_preset_bundle_path,
};

pub(in crate::preset) fn export_preset_from(
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

pub(in crate::preset) fn export_preset_from_with_confirm(
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

    // One snapshot pins every validation and archive read to the same config directory
    let mut source = ExportSourceSnapshot::capture(config_dir)?;
    let mut config = source.config().clone();
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
    let active_theme_paths = source.capture_active_files(config_dir, &theme_files)?;
    let existing_theme_files = active_theme_paths
        .iter()
        .map(|relative| config_dir.join(relative))
        .collect::<Vec<_>>();
    let mut selected_paths = vec![PathBuf::from("config.toml")];
    selected_paths.extend(active_theme_paths);
    let css_asset_paths = collect_local_css_asset_paths_from_captures(
        config_dir,
        &existing_theme_files,
        source.captures(),
    )?;
    let css_asset_files = css_asset_paths
        .iter()
        .map(|relative| config_dir.join(relative))
        .collect::<Vec<_>>();
    selected_paths.extend(source.capture_active_files(config_dir, &css_asset_files)?);
    let command_script_paths = collect_command_references_from_config(&config)
        .into_iter()
        .filter_map(|reference| resolve_command_path_token(config_dir, &reference.command))
        .filter(|path| path.is_file())
        .filter_map(|path| path.strip_prefix(config_dir).ok().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    // Direct config commands can source small helper libraries that are just as required as the entry script
    // Resolve that closure before collection so a preset remains usable on a clean installation
    let script_dependencies =
        collect_script_dependency_closure_from_root(source.root_fd(), &command_script_paths)?;
    selected_paths.extend(script_dependencies.paths.iter().cloned());
    source.extend_captures(script_dependencies.captures);
    let icon_paths = collect_existing_icon_assets(source.config_bytes(), config_dir)?;
    let icon_files = icon_paths
        .iter()
        .map(|relative| config_dir.join(relative))
        .collect::<Vec<_>>();
    selected_paths.extend(source.capture_active_files(config_dir, &icon_files)?);
    selected_paths.sort();
    selected_paths.dedup();

    let mut collected = collect_selected_config_files_from_root(
        source.root_fd(),
        config_dir,
        &selected_paths,
        Some(output_path),
        &exclusions,
        source.captures(),
    )?;
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
    // A renamed config root would otherwise publish a stale snapshot under a new live path
    source.ensure_live_root(config_dir)?;
    write_bundle(output_path, &manifest, &collected).context("write preset bundle")?;

    Ok(ExportSummary {
        bundle_path: output_path.to_path_buf(),
        file_count: collected.files.len(),
        skipped_symlinks: collected.skipped_symlinks,
        skipped_non_regular: collected.skipped_non_regular,
    })
}
