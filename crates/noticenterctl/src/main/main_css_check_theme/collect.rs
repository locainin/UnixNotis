use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use unixnotis_core::Config;

use crate::preset::command_rules::{
    collect_host_specific_command_paths, collect_outside_command_paths,
};
use crate::preset::css_asset_refs::collect_external_css_asset_refs_from_paths;

use super::super::main_css_check_files::{collect_css_files, format_display_path};
use super::super::main_css_check_report::{
    CssCheckActiveFile, CssCheckCategory, CssCheckDiagnostic,
};
use super::model::CssCheckInputs;
use super::paths::{
    dedupe_key_for_theme_file, has_css_extension, join_config_keys, normalize_target_key,
    theme_targets,
};

pub(in super::super) fn collect_css_check_inputs(
    config_dir: &Path,
    display_root: &str,
) -> Result<CssCheckInputs> {
    // css-check always reflects the live config tree, not a detached preset view
    let config_path = Config::default_config_path().context("resolve config path")?;
    let config = Config::load_default().context("load config for active theme paths")?;
    collect_css_check_inputs_from(config_dir, display_root, &config_path, &config)
}

pub(in super::super) fn collect_css_check_inputs_from(
    config_dir: &Path,
    display_root: &str,
    config_path: &Path,
    config: &Config,
) -> Result<CssCheckInputs> {
    let config_display = format_display_path(config_dir, display_root, config_path);

    // css-check should follow the same theme path resolution as the UI
    let theme_paths = config
        .resolve_theme_paths_from(config_dir)
        .context("resolve theme paths for css-check")?;

    // Extra root css files are tracked so the report can explain what was skipped
    let root_css_files = collect_css_files(config_dir)?;
    let root_css_set: HashSet<_> = root_css_files.iter().cloned().collect();

    let targets = theme_targets(theme_paths);
    let mut diagnostics = Vec::new();
    let mut notes = Vec::new();
    let mut files = Vec::new();
    let mut active_files = Vec::new();
    let mut seen_files = HashSet::new();
    let mut duplicate_slots: BTreeMap<_, Vec<&'static str>> = BTreeMap::new();
    let mut outside_root = 0usize;
    let mut non_css_targets = 0usize;

    for target in &targets {
        // Slot activity is reported even when the file later gets skipped or deduped
        active_files.push(CssCheckActiveFile {
            slot_name: target.config_key,
            display_path: target.display_path(config_dir, display_root),
        });
        if !target.path.starts_with(config_dir) {
            // Outside theme files work, but they make the setup less portable
            outside_root += 1;
        }
        if !has_css_extension(&target.path) {
            // The UI still loads these files, so css-check should not skip them
            non_css_targets += 1;
        }

        duplicate_slots
            .entry(normalize_target_key(&target.path))
            .or_default()
            .push(target.config_key);

        if !target.path.exists() {
            diagnostics.push(CssCheckDiagnostic::warning(
                CssCheckCategory::Theme,
                target.display_path(config_dir, display_root),
                format!(
                    "configured {} target is missing; UnixNotis will create a default theme file there on startup, so css-check is validating less than the live UI expects",
                    target.slot_name
                ),
            ));
            continue;
        }

        if !target.path.is_file() {
            diagnostics.push(CssCheckDiagnostic::warning(
                CssCheckCategory::Theme,
                target.display_path(config_dir, display_root),
                format!(
                    "configured {} target is not a regular file",
                    target.slot_name
                ),
            ));
            continue;
        }

        // One parse and one lint pass per real file is enough
        //
        // Canonical paths collapse symlink aliases and repeated logical paths
        // back onto the same on-disk file before GTK sees them
        let dedupe_key = dedupe_key_for_theme_file(&target.path);
        if seen_files.insert(dedupe_key) {
            files.push(target.path.clone());
        }
    }

    // Command diagnostics stay separate from css file diagnostics so the report is easier to read
    let outside_commands = collect_outside_command_paths(config_dir, config);
    let outside_command_paths = outside_commands.len();
    for command in outside_commands {
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            config_display.clone(),
            format!(
                "{} points outside {display_root}; shared presets should keep explicit command paths inside the UnixNotis config directory ({})",
                command.slot, command.command
            ),
        ));
    }

    // Command paths can still leak host-local values even when config.toml stays inside the root
    let host_specific_commands = collect_host_specific_command_paths(config_dir, config);
    let host_specific_command_paths = host_specific_commands.len();
    for command in host_specific_commands {
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            config_display.clone(),
            format!(
                "{} uses a host-local command path inside {display_root}; export should rewrite it before sharing: {}",
                command.slot, command.command
            ),
        ));
    }

    // CSS can still pull assets from outside the config root
    let external_css_refs = collect_external_css_asset_refs_from_paths(config_dir, &files)?;
    let external_css_asset_refs = external_css_refs.len();
    let remote_css_asset_refs = external_css_refs
        .iter()
        .filter(|asset_ref| asset_ref.reason == "remote url")
        .count();
    let local_external_css_asset_refs = external_css_asset_refs - remote_css_asset_refs;
    for asset_ref in external_css_refs {
        // Asset findings are tied to the css file that referenced them
        let message = format_external_css_asset_ref_message(display_root, &asset_ref);
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            format_display_path(config_dir, display_root, &asset_ref.css_file),
            message,
        ));
    }

    for slots in duplicate_slots.values() {
        if slots.len() < 2 {
            continue;
        }

        // One file loaded into multiple theme slots can look much stronger than expected
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            config_display.clone(),
            format!(
                "{} all resolve to the same file; that stylesheet will be loaded into multiple UnixNotis theme slots",
                join_config_keys(slots)
            ),
        ));
    }

    let configured_existing: HashSet<_> = files.iter().cloned().collect();
    let skipped_extra_css = root_css_set
        .iter()
        .filter(|path| !configured_existing.contains(*path))
        .count();

    if outside_root > 0 {
        notes.push(format!(
            "{outside_root} configured theme file(s) live outside {display_root} and were checked directly"
        ));
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            config_display.clone(),
            format!(
                "{outside_root} configured theme file(s) point outside {display_root}; that makes the setup less portable and means those files are loaded from outside the UnixNotis config directory"
            ),
        ));
    }
    if outside_command_paths > 0 {
        notes.push(format!(
            "{outside_command_paths} configured command path(s) point outside {display_root}"
        ));
    }
    if host_specific_command_paths > 0 {
        notes.push(format!(
            "{host_specific_command_paths} configured command path(s) use host-local config-root paths"
        ));
    }

    // Keep the live css-check notes aligned with the preset safety prompts
    if local_external_css_asset_refs > 0 {
        notes.push(format!(
            "{local_external_css_asset_refs} css asset reference(s) leave {display_root}"
        ));
    }
    if remote_css_asset_refs > 0 {
        notes.push(format!(
            "{remote_css_asset_refs} css asset reference(s) use remote URLs"
        ));
    }
    if non_css_targets > 0 {
        notes.push(format!(
            "{non_css_targets} configured theme file(s) do not end in .css and were checked because config.toml points to them"
        ));
    }
    if skipped_extra_css > 0 {
        notes.push(format!(
            "{skipped_extra_css} extra css file(s) under {display_root} were skipped because config.toml does not reference them"
        ));
    }

    // Sorting here keeps downstream lint and parse output deterministic
    files.sort();
    Ok(CssCheckInputs {
        files,
        active_files,
        notes,
        diagnostics,
    })
}

fn format_external_css_asset_ref_message(
    display_root: &str,
    asset_ref: &crate::preset::css_asset_refs::ExternalCssAssetRef,
) -> String {
    if asset_ref.reason == "remote url" {
        return format!(
            "css asset reference uses a remote URL: {}",
            asset_ref.asset_ref
        );
    }

    // Local-but-external paths should still explain why they escaped the root
    format!(
        "css asset reference leaves {display_root}: {} ({})",
        asset_ref.asset_ref, asset_ref.reason
    )
}
