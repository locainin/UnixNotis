//! Active theme target discovery and path sanity checks for css-check

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::{Config, ThemePaths};

use crate::preset::command_rules::{
    collect_host_specific_command_paths, collect_outside_command_paths,
};
use crate::preset::css_asset_refs::collect_external_css_asset_refs_from_paths;

use super::main_css_check_files::{collect_css_files, format_display_path};
use super::main_css_check_report::{CssCheckActiveFile, CssCheckCategory, CssCheckDiagnostic};

pub(super) struct CssCheckInputs {
    pub(super) files: Vec<PathBuf>,
    pub(super) active_files: Vec<CssCheckActiveFile>,
    pub(super) notes: Vec<String>,
    pub(super) diagnostics: Vec<CssCheckDiagnostic>,
}

struct ThemeTarget {
    slot_name: &'static str,
    config_key: &'static str,
    path: PathBuf,
}

impl ThemeTarget {
    fn display_path(&self, config_dir: &Path, display_root: &str) -> String {
        format_display_path(config_dir, display_root, &self.path)
    }
}

pub(super) fn collect_css_check_inputs(
    config_dir: &Path,
    display_root: &str,
) -> Result<CssCheckInputs> {
    let config_path = Config::default_config_path().context("resolve config path")?;
    let config = Config::load_default().context("load config for active theme paths")?;
    collect_css_check_inputs_from(config_dir, display_root, &config_path, &config)
}

fn collect_css_check_inputs_from(
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
    let root_css_set: HashSet<PathBuf> = root_css_files.iter().cloned().collect();

    let targets = theme_targets(theme_paths);
    let mut diagnostics = Vec::new();
    let mut notes = Vec::new();
    let mut files = Vec::new();
    let mut active_files = Vec::new();
    let mut seen_files = HashSet::new();
    let mut duplicate_slots: BTreeMap<PathBuf, Vec<&'static str>> = BTreeMap::new();
    let mut outside_root = 0usize;
    let mut non_css_targets = 0usize;

    for target in &targets {
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
                "THEME001",
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
                "THEME002",
                target.display_path(config_dir, display_root),
                format!(
                    "configured {} target is not a regular file",
                    target.slot_name
                ),
            ));
            continue;
        }

        // One parse and one lint pass per unique file is enough
        if seen_files.insert(target.path.clone()) {
            files.push(target.path.clone());
        }
    }

    let outside_commands = collect_outside_command_paths(config_dir, config);
    let outside_command_paths = outside_commands.len();
    for command in outside_commands {
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            "THEME005",
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
            "THEME006",
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
    for asset_ref in external_css_refs {
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            "THEME007",
            format_display_path(config_dir, display_root, &asset_ref.css_file),
            format!(
                "css asset reference points outside {display_root}: {} ({})",
                asset_ref.asset_ref, asset_ref.reason
            ),
        ));
    }

    for slots in duplicate_slots.values() {
        if slots.len() < 2 {
            continue;
        }

        // One file loaded into multiple theme slots can look much stronger than expected
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Theme,
            "THEME003",
            config_display.clone(),
            format!(
                "{} all resolve to the same file; that stylesheet will be loaded into multiple UnixNotis theme slots",
                join_config_keys(slots)
            ),
        ));
    }

    let configured_existing: HashSet<PathBuf> = files.iter().cloned().collect();
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
            "THEME004",
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
    if external_css_asset_refs > 0 {
        notes.push(format!(
            "{external_css_asset_refs} css asset reference(s) point outside {display_root}"
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

    files.sort();
    Ok(CssCheckInputs {
        files,
        active_files,
        notes,
        diagnostics,
    })
}

fn theme_targets(theme_paths: ThemePaths) -> [ThemeTarget; 5] {
    [
        ThemeTarget {
            slot_name: "base css",
            config_key: "[theme].base_css",
            path: theme_paths.base_css,
        },
        ThemeTarget {
            slot_name: "panel css",
            config_key: "[theme].panel_css",
            path: theme_paths.panel_css,
        },
        ThemeTarget {
            slot_name: "popup css",
            config_key: "[theme].popup_css",
            path: theme_paths.popup_css,
        },
        ThemeTarget {
            slot_name: "widgets css",
            config_key: "[theme].widgets_css",
            path: theme_paths.widgets_css,
        },
        ThemeTarget {
            slot_name: "media css",
            config_key: "[theme].media_css",
            path: theme_paths.media_css,
        },
    ]
}

fn has_css_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("css"))
        .unwrap_or(false)
}

fn normalize_target_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn join_config_keys(keys: &[&'static str]) -> String {
    keys.iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
#[path = "main_css_check_theme_tests.rs"]
mod tests;
