//! Import validation helpers for hostile preset content
//!
//! These checks run before import writes anything to disk so
//! crafted bundles fail early instead of escaping through later setup steps

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use toml::Value;
use unixnotis_core::{Config, ThemePaths};

use super::super::command_rules::{
    validate_command_paths_in_config_bytes, validate_config_command_paths_stay_in_root,
};
use super::super::pathing::normalize_lexical_path;
use crate::preset::archive::BundleFile;

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

pub(super) fn validate_imported_theme_paths_stay_in_root(
    config_dir: &Path,
    config_bytes: &[u8],
) -> Result<()> {
    // The bundle config is trusted during post-import setup, so its theme targets must stay local
    let config_text =
        std::str::from_utf8(config_bytes).context("preset config.toml is not valid UTF-8")?;
    let config: Config =
        toml::from_str(config_text).context("parse bundled config.toml for import validation")?;
    validate_config_theme_paths_stay_in_root(config_dir, &config)
}

pub(super) fn validate_imported_command_paths_stay_in_root(
    config_dir: &Path,
    config_bytes: &[u8],
) -> Result<()> {
    // Preset import should reject explicit command paths that escape the shared config root
    validate_command_paths_in_config_bytes(config_dir, config_bytes, "preset import blocked")
}

pub(super) fn validate_config_theme_paths_stay_in_root(
    config_dir: &Path,
    config: &Config,
) -> Result<()> {
    // Resolve against the target config root because that is where import will later materialize CSS files
    let theme_paths = config
        .resolve_theme_paths_from(config_dir)
        .context("resolve bundled theme paths for import validation")?;
    validate_resolved_theme_paths_stay_in_root(config_dir, &theme_paths)
}

pub(super) fn validate_config_command_paths_for_import(
    config_dir: &Path,
    config: &Config,
) -> Result<()> {
    // Live config revalidation closes the kept-local config chain after import writes land
    validate_config_command_paths_stay_in_root(config_dir, config, "preset import blocked")
}

pub(super) fn collect_imported_exec_content(
    config_bytes: &[u8],
    bundle_files: &[BundleFile],
) -> Result<ImportedExecContent> {
    // Only explicit command keys from the bundle count here
    // Runtime defaults should not make ordinary presets fail import
    let commands = collect_explicit_exec_commands_from_config_bytes(config_bytes)?;
    let files = bundle_files
        .iter()
        // The review should show every runnable payload before anything is written
        .filter(|file| import_file_looks_executable(file))
        .map(|file| ImportedExecFile {
            relative_path: file.relative_path.clone(),
            contents: file.contents.clone(),
            mode: file.mode,
        })
        .collect();

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
    let value: Value =
        toml::from_str(config_text).context("parse bundled config.toml for exec validation")?;
    let mut commands = Vec::new();
    // Walking the parsed value keeps nested widget tables and plugin blocks covered
    collect_explicit_exec_commands("", &value, &mut commands);
    Ok(commands)
}

fn collect_explicit_exec_commands(
    prefix: &str,
    value: &Value,
    commands: &mut Vec<ImportedExecCommand>,
) {
    match value {
        Value::Table(table) => {
            for (key, child) in table {
                let next = join_toml_slot(prefix, key);
                // A direct string command is enough to make the preset runnable
                if key_is_exec_slot(key) {
                    if let Some(command) = child.as_str().filter(|value| !value.trim().is_empty()) {
                        commands.push(ImportedExecCommand {
                            slot: next.clone(),
                            command: command.trim().to_string(),
                        });
                    }
                }
                // Walk deeper so nested plugin tables and widget arrays are covered too
                collect_explicit_exec_commands(&next, child, commands);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let next = format!("{prefix}[{index}]");
                // Arrays carry indexed widget rows, so keep their slot names stable in errors
                collect_explicit_exec_commands(&next, child, commands);
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

fn key_is_exec_slot(key: &str) -> bool {
    // This stays explicit so import review only flags fields that are known to run something
    matches!(
        key,
        "cmd"
            | "command"
            | "get_cmd"
            | "set_cmd"
            | "toggle_cmd"
            | "watch_cmd"
            | "state_cmd"
            | "on_cmd"
            | "off_cmd"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        collect_imported_exec_content, validate_imported_command_paths_stay_in_root,
        validate_imported_theme_paths_stay_in_root,
    };
    use crate::preset::archive::BundleFile;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root(name: &str) -> PathBuf {
        // Unique absolute paths keep these lexical checks stable under parallel cargo runs
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "unixnotis-preset-import-checks-{}-{}-{}",
            name, stamp, serial
        ))
    }

    #[test]
    fn imported_theme_checks_reject_parent_traversal_targets() {
        // `../` theme paths should be treated the same as any other root escape
        let config_dir = temp_root("relative-escape");
        let config = b"[theme]\nbase_css = \"../escaped-base.css\"\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n";

        let error = validate_imported_theme_paths_stay_in_root(&config_dir, config)
            .expect_err("reject relative theme escape");

        assert!(error
            .to_string()
            .contains("tries to leave the UnixNotis config directory"));
    }

    #[test]
    fn imported_command_checks_reject_absolute_plugin_command() {
        // Shared presets should not carry explicit command paths that leave the config root
        let config_dir = temp_root("outside-command");
        let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\n[widgets.stats.plugin]\napi_version = 1\ncommand = \"/tmp/outside-plugin\"\n";

        let error = validate_imported_command_paths_stay_in_root(&config_dir, config)
            .expect_err("reject outside command path");

        assert!(error
            .to_string()
            .contains("points outside the UnixNotis config directory"));
    }

    #[test]
    fn imported_exec_collection_finds_command_bearing_config() {
        let config = br#"
[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/check.sh"
"#;

        let content = collect_imported_exec_content(config, &[]).expect("collect exec content");

        assert_eq!(content.commands.len(), 1);
        assert_eq!(content.commands[0].slot, "widgets.stats[0].cmd");
        assert_eq!(content.commands[0].command, "scripts/check.sh");
    }

    #[test]
    fn imported_exec_collection_finds_script_payloads() {
        let config = br#"
[theme]
base_css = "base.css"
"#;
        let bundle_files = vec![BundleFile {
            relative_path: PathBuf::from("scripts/demo-widget"),
            contents: b"#!/bin/sh\necho ok\n".to_vec(),
            mode: 0o755,
        }];

        let content =
            collect_imported_exec_content(config, &bundle_files).expect("collect script payload");

        assert_eq!(content.files.len(), 1);
        assert_eq!(
            content.files[0].relative_path,
            PathBuf::from("scripts/demo-widget")
        );
    }

    #[test]
    fn imported_exec_collection_keeps_command_and_script_details() {
        let config = br#"
[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/check.sh"
"#;
        let bundle_files = vec![BundleFile {
            relative_path: PathBuf::from("scripts/check.sh"),
            contents: b"#!/bin/sh\necho ok\n".to_vec(),
            mode: 0o755,
        }];

        let content =
            collect_imported_exec_content(config, &bundle_files).expect("collect trusted exec");

        assert_eq!(content.commands.len(), 1);
        assert_eq!(content.files.len(), 1);
    }
}
