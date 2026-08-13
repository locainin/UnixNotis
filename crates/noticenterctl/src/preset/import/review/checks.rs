//! Import validation helpers for hostile preset content
//!
//! These checks run before import writes anything to disk so
//! crafted bundles fail early instead of escaping through later setup steps

use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use std::path::Path;
use toml::Value;
use unixnotis_core::{
    validate_icon_asset_contents, validate_icon_asset_reference, Config, ThemePaths,
};

use super::super::super::archive::BundleFile;

use super::super::super::command_rules::{
    collect_command_references_from_config, validate_command_paths_in_config_bytes,
    validate_config_command_paths_stay_in_root,
};
use super::super::super::pathing::normalize_lexical_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::preset) struct ImportedExecContent {
    // Command slots are shown back to the user before import continues
    pub(in crate::preset) commands: Vec<ImportedExecCommand>,
    // Bundled files are kept with bytes so the review pager can show the real payload
    pub(in crate::preset) files: Vec<ImportedExecFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::preset) struct ImportedExecCommand {
    // Slot path keeps the warning tied to the exact config field
    pub(in crate::preset) slot: String,
    pub(in crate::preset) command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::preset) struct ImportedExecFile {
    // Relative path inside the bundle is what the import will materialize
    pub(in crate::preset) relative_path: std::path::PathBuf,
    pub(in crate::preset) contents: Vec<u8>,
    pub(in crate::preset) mode: u32,
}

pub(in crate::preset) fn validate_imported_theme_paths_stay_in_root(
    config_dir: &Path,
    config_bytes: &[u8],
) -> Result<()> {
    // The bundle config is trusted during post-import setup, so its theme targets must stay local
    let config_text =
        std::str::from_utf8(config_bytes).context("preset config.toml is not valid UTF-8")?;
    let config =
        Config::parse(config_text).context("parse bundled config.toml for import validation")?;
    validate_config_theme_paths_stay_in_root(config_dir, &config)
}

pub(in crate::preset) fn validate_imported_command_paths_stay_in_root(
    config_dir: &Path,
    config_bytes: &[u8],
) -> Result<()> {
    // Preset import should reject explicit command paths that escape the shared config root
    validate_command_paths_in_config_bytes(config_dir, config_bytes, "preset import blocked")
}

pub(in crate::preset) fn validate_imported_icon_assets(
    config_bytes: &[u8],
    bundle_files: &[BundleFile],
) -> Result<()> {
    let config_text =
        std::str::from_utf8(config_bytes).context("preset config.toml is not valid UTF-8")?;
    let value: Value =
        toml::from_str(config_text).context("parse bundled config.toml for icon asset checks")?;
    let mut assets = Vec::new();
    collect_explicit_icon_assets("", &value, &mut assets);

    for asset in assets {
        validate_icon_asset_reference(&asset.asset).with_context(|| {
            format!(
                "preset import blocked because {} has an invalid icon_asset",
                asset.slot
            )
        })?;
        let Some(file) = bundle_files
            .iter()
            .find(|file| file.relative_path == Path::new(&asset.asset))
        else {
            // Optional missing assets preserve the documented theme-icon fallback behavior
            continue;
        };
        if file.mode & 0o111 != 0 {
            return Err(anyhow!(
                "preset import blocked because {} references executable icon asset {}",
                asset.slot,
                asset.asset
            ));
        }
        validate_icon_asset_contents(&asset.asset, &file.contents).with_context(|| {
            format!(
                "preset import blocked because {} references unsafe icon asset {}",
                asset.slot, asset.asset
            )
        })?;
    }
    Ok(())
}

pub(in crate::preset) fn validate_config_theme_paths_stay_in_root(
    config_dir: &Path,
    config: &Config,
) -> Result<()> {
    // Resolve against the target config root because that is where import will later materialize CSS files
    let theme_paths = config
        .resolve_theme_paths_from(config_dir)
        .context("resolve bundled theme paths for import validation")?;
    validate_resolved_theme_paths_stay_in_root(config_dir, &theme_paths)
}

pub(in crate::preset) fn validate_config_command_paths_for_import(
    config_dir: &Path,
    config: &Config,
) -> Result<()> {
    // Live config revalidation closes the kept-local config chain after import writes land
    validate_config_command_paths_stay_in_root(config_dir, config, "preset import blocked")
}

pub(in crate::preset) fn collect_imported_exec_content(
    config_bytes: &[u8],
    bundle_files: &[BundleFile],
) -> Result<ImportedExecContent> {
    // Use the runtime configuration model so unknown lookalike keys cannot consume review space
    let commands = collect_explicit_exec_commands_from_config_bytes(config_bytes)?;
    let has_executable_file = bundle_files.iter().any(import_file_looks_executable);
    // A command or script can pass any neighboring file to another interpreter or loader
    let files = if commands.is_empty() && !has_executable_file {
        Vec::new()
    } else {
        bundle_files
            .iter()
            .map(|file| ImportedExecFile {
                relative_path: file.relative_path.clone(),
                contents: file.contents.clone(),
                mode: file.mode,
            })
            .collect()
    };

    Ok(ImportedExecContent { commands, files })
}

fn validate_resolved_theme_paths_stay_in_root(
    config_dir: &Path,
    theme_paths: &ThemePaths,
) -> Result<()> {
    // Normalize the root first so `../` tricks are compared against the real final location
    let normalized_root = normalize_lexical_path(config_dir);

    for (slot_name, path) in [
        ("base_css", &theme_paths.base_css),
        ("panel_css", &theme_paths.panel_css),
        ("popup_css", &theme_paths.popup_css),
        ("widgets_css", &theme_paths.widgets_css),
        ("media_css", &theme_paths.media_css),
    ] {
        // Normalize each target so lexical parent traversal cannot hide outside writes
        let normalized_path = normalize_lexical_path(path);
        // Absolute or host-specific theme targets would let post-import setup escape the config root
        if !normalized_path.starts_with(&normalized_root) {
            return Err(anyhow!(
                "preset import blocked because theme.{} tries to leave the UnixNotis config directory: {}",
                slot_name,
                path.display()
            ));
        }
    }

    Ok(())
}

fn import_file_looks_executable(file: &BundleFile) -> bool {
    // Explicit execute bits are the clearest signal that a preset carries runnable payload
    if file.mode & 0o111 != 0 {
        return true;
    }

    // Script roots are treated as executable content even when the bundle did not preserve mode
    // A shell-based widget command can run these files directly through `sh path`
    file.relative_path.starts_with("scripts")
}

fn collect_explicit_exec_commands_from_config_bytes(
    config_bytes: &[u8],
) -> Result<Vec<ImportedExecCommand>> {
    let config_text =
        std::str::from_utf8(config_bytes).context("preset config.toml is not valid UTF-8")?;
    let document: Value =
        toml::from_str(config_text).context("parse bundled config.toml for exec validation")?;
    // The normal parser applies the same migrations, limits, and cleanup as the running UI
    let report = Config::parse_with_report(config_text)
        .context("parse bundled config.toml through the runtime configuration model")?;
    let explicit_slots = collect_known_explicit_exec_slots(&document);

    Ok(collect_command_references_from_config(&report.config)
        .into_iter()
        // Defaults are useful at runtime but should not make a data-only preset require approval
        .filter(|reference| explicit_slots.contains(&reference.slot))
        .map(|reference| ImportedExecCommand {
            slot: reference.slot,
            command: reference.command.display_lossy(),
        })
        .collect())
}

fn collect_known_explicit_exec_slots(document: &Value) -> HashSet<String> {
    let mut slots = HashSet::new();
    let Some(widgets) = document.get("widgets").and_then(Value::as_table) else {
        return slots;
    };

    // Slider commands live in fixed tables rather than user-defined plugin namespaces
    for slider_name in ["volume", "brightness"] {
        let Some(slider) = widgets.get(slider_name).and_then(Value::as_table) else {
            continue;
        };
        collect_present_table_fields(
            slider,
            &format!("widgets.{slider_name}"),
            &["get_cmd", "set_cmd", "toggle_cmd", "watch_cmd"],
            &mut slots,
        );
    }

    collect_present_widget_fields(
        widgets,
        "toggles",
        &["state_cmd", "toggle_cmd", "on_cmd", "off_cmd", "watch_cmd"],
        &mut slots,
    );
    collect_present_widget_fields(widgets, "stats", &["cmd"], &mut slots);
    collect_present_plugin_fields(widgets, "stats", &mut slots);
    collect_present_widget_fields(widgets, "cards", &["cmd"], &mut slots);
    collect_present_plugin_fields(widgets, "cards", &mut slots);
    slots
}

fn collect_present_widget_fields(
    widgets: &toml::Table,
    collection_name: &str,
    command_fields: &[&str],
    slots: &mut HashSet<String>,
) {
    let Some(items) = widgets.get(collection_name).and_then(Value::as_array) else {
        return;
    };

    for (index, item) in items.iter().enumerate() {
        let Some(table) = item.as_table() else {
            continue;
        };
        let base = format!("widgets.{collection_name}[{index}]");
        collect_present_table_fields(table, &base, command_fields, slots);
    }
}

fn collect_present_plugin_fields(
    widgets: &toml::Table,
    collection_name: &str,
    slots: &mut HashSet<String>,
) {
    let Some(items) = widgets.get(collection_name).and_then(Value::as_array) else {
        return;
    };

    for (index, item) in items.iter().enumerate() {
        if item
            .get("plugin")
            .and_then(Value::as_table)
            .is_some_and(|plugin| plugin.contains_key("command"))
        {
            slots.insert(format!("widgets.{collection_name}[{index}].plugin.command"));
        }
    }
}

fn collect_present_table_fields(
    table: &toml::Table,
    base: &str,
    fields: &[&str],
    slots: &mut HashSet<String>,
) {
    for field in fields {
        // Presence is enough here because typed parsing already checked the value type
        if table.contains_key(*field) {
            slots.insert(format!("{base}.{field}"));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportedIconAsset {
    slot: String,
    asset: String,
}

fn collect_explicit_icon_assets(prefix: &str, value: &Value, assets: &mut Vec<ImportedIconAsset>) {
    match value {
        Value::Table(table) => {
            for (key, child) in table {
                let next = join_toml_slot(prefix, key);
                if key == "icon_asset" {
                    if let Some(asset) = child.as_str().filter(|value| !value.trim().is_empty()) {
                        assets.push(ImportedIconAsset {
                            slot: next.clone(),
                            asset: asset.trim().to_string(),
                        });
                    }
                }
                collect_explicit_icon_assets(&next, child, assets);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let next = format!("{prefix}[{index}]");
                collect_explicit_icon_assets(&next, child, assets);
            }
        }
        _ => {}
    }
}

fn join_toml_slot(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}
