//! Preset inspect flow for printing bundle contents and command references
//!
//! Inspect is read-only and is meant to answer two questions quickly:
//! what files are inside the preset, and what command-bearing config fields it carries

use anyhow::{Context, Result};
use std::path::Path;
use unixnotis_core::{
    util, Config, MAX_CARD_WIDGETS, MAX_STAT_WIDGETS, MAX_TOGGLE_WIDGETS, MAX_TOTAL_WIDGETS,
};

use super::archive::read_bundle;
use super::command_rules::{
    collect_command_references_from_config, collect_host_specific_command_paths,
    collect_outside_command_paths,
};
use super::css_asset_refs::collect_external_css_asset_refs_from_bundle;
use super::pathing::{
    normalize_lexical_path, resolve_cli_bundle_path, validate_preset_bundle_path,
};

pub(super) fn run_inspect(input_path: &Path) -> Result<()> {
    // CLI inspect accepts a missing extension and can append it after confirmation
    let input_path = resolve_cli_bundle_path(input_path)?;
    // CLI path just prints the already-formatted report
    let report = inspect_preset_at(&input_path)?;
    print!("{report}");
    Ok(())
}

pub(super) fn inspect_preset_at(input_path: &Path) -> Result<String> {
    validate_preset_bundle_path(input_path)?;
    // Inspect uses the same reader as import so both commands see the same validation rules
    let bundle = read_bundle(input_path).context("read preset bundle for inspect")?;

    let mut out = String::new();
    out.push_str(&format!(
        "preset: {}\n",
        safe_report_value(&bundle.manifest.bundle_name)
    ));
    out.push_str(&format!(
        "format version: {}\n",
        bundle.manifest.format_version
    ));
    out.push_str(&format!(
        "exported at: {}\n",
        safe_report_value(&bundle.manifest.exported_at)
    ));
    out.push_str(&format!(
        "tool version: {}\n",
        safe_report_value(&bundle.manifest.tool_version)
    ));
    out.push_str(&format!("files: {}\n", bundle.manifest.files.len()));
    out.push_str(&format!("assets: {}\n", yes_no(bundle.manifest.has_assets)));
    out.push_str(&format!(
        "scripts: {}\n",
        yes_no(bundle.manifest.has_scripts)
    ));

    if let Some(config_file) = bundle
        .files
        .iter()
        .find(|file| file.relative_path == Path::new("config.toml"))
    {
        // config.toml is parsed from the bundle bytes without touching the local config root
        match std::str::from_utf8(&config_file.contents) {
            Ok(contents) => match Config::parse(contents) {
                Ok(config) => {
                    append_widget_counts(&mut out, requested_widget_counts(contents));
                    let commands = collect_command_references_from_config(&config);
                    out.push_str(&format!("command refs: {}\n", commands.len()));
                    if commands.is_empty() {
                        out.push_str("  none\n");
                    } else {
                        for command in commands {
                            out.push_str(&format!(
                                "  - {} = {}\n",
                                safe_report_value(&command.slot),
                                safe_report_value(&command.command)
                            ));
                        }
                    }

                    // Inspect has no live config root, so this placeholder keeps the warning shape stable
                    let outside_paths = collect_outside_command_paths(
                        Path::new("$XDG_CONFIG_HOME/unixnotis"),
                        &config,
                    );
                    out.push_str(&format!("command path warnings: {}\n", outside_paths.len()));
                    if outside_paths.is_empty() {
                        out.push_str("  none\n");
                    } else {
                        for warning in outside_paths {
                            out.push_str(&format!(
                                "  - {} points outside the config root: {}\n",
                                safe_report_value(&warning.slot),
                                safe_report_value(&warning.command)
                            ));
                        }
                    }

                    let leaked_paths = collect_host_specific_command_paths(
                        Path::new("$XDG_CONFIG_HOME/unixnotis"),
                        &config,
                    );
                    out.push_str(&format!(
                        "host-specific command paths: {}\n",
                        leaked_paths.len()
                    ));
                    if leaked_paths.is_empty() {
                        out.push_str("  none\n");
                    } else {
                        for leak in leaked_paths {
                            out.push_str(&format!(
                                "  - {} uses a host-local config path: {}\n",
                                safe_report_value(&leak.slot),
                                safe_report_value(&leak.command)
                            ));
                        }
                    }

                    // Show theme slot escapes in inspect output
                    let theme_warnings = collect_theme_path_warnings(&config);
                    out.push_str(&format!("theme path warnings: {}\n", theme_warnings.len()));
                    if theme_warnings.is_empty() {
                        out.push_str("  none\n");
                    } else {
                        for warning in theme_warnings {
                            out.push_str(&format!("  - {}\n", safe_report_value(&warning)));
                        }
                    }
                }
                Err(err) => {
                    out.push_str(&format!(
                        "command refs: unavailable ({})\n",
                        safe_report_value(&err.to_string())
                    ));
                }
            },
            Err(err) => {
                out.push_str(&format!(
                    "command refs: unavailable ({})\n",
                    safe_report_value(&err.to_string())
                ));
            }
        }
    } else {
        out.push_str("command refs: unavailable (config.toml missing)\n");
    }

    // Read CSS warnings from bundle bytes only
    let css_asset_warnings = collect_external_css_asset_refs_from_bundle(
        Path::new("$XDG_CONFIG_HOME/unixnotis"),
        &bundle.files,
    );
    out.push_str(&format!(
        "css asset path warnings: {}\n",
        css_asset_warnings.len()
    ));
    if css_asset_warnings.is_empty() {
        out.push_str("  none\n");
    } else {
        for warning in css_asset_warnings {
            out.push_str(&format!(
                "  - {} -> {} ({})\n",
                safe_report_path(warning.css_file.as_path()),
                safe_report_value(&warning.asset_ref),
                safe_report_value(&warning.reason)
            ));
        }
    }

    out.push_str("file list:\n");
    for file in &bundle.manifest.files {
        out.push_str(&format!("  - {}\n", safe_report_value(&file.path)));
    }
    Ok(out)
}

fn requested_widget_counts(contents: &str) -> (usize, usize, usize) {
    let Ok(document) = contents.parse::<toml::Value>() else {
        return (0, 0, 0);
    };
    let widgets = document.get("widgets");
    let count = |key: &str| {
        widgets
            .and_then(|value| value.get(key))
            .and_then(toml::Value::as_array)
            .map_or(0, Vec::len)
    };
    (count("toggles"), count("stats"), count("cards"))
}

fn append_widget_counts(out: &mut String, counts: (usize, usize, usize)) {
    let (toggles, stats, cards) = counts;
    let total = toggles.saturating_add(stats).saturating_add(cards);
    out.push_str(&format!(
        "widgets requested: {total} (toggles={toggles}, stats={stats}, cards={cards})\n"
    ));

    // Inspection is read-only, so report the unsafe shape before import sanitizes it
    if toggles > MAX_TOGGLE_WIDGETS
        || stats > MAX_STAT_WIDGETS
        || cards > MAX_CARD_WIDGETS
        || total > MAX_TOTAL_WIDGETS
    {
        out.push_str(&format!(
            "widget limit warning: runtime keeps at most {MAX_TOTAL_WIDGETS} total \
(toggles={MAX_TOGGLE_WIDGETS}, stats={MAX_STAT_WIDGETS}, cards={MAX_CARD_WIDGETS})\n"
        ));
    }
}

const fn yes_no(value: bool) -> &'static str {
    // Small helper keeps inspect output predictable and grep-friendly
    if value {
        "yes"
    } else {
        "no"
    }
}

fn safe_report_value(value: &str) -> String {
    // Presets can come from someone else, so inspect output must not emit raw terminal controls
    util::sanitize_log_value(value, util::diagnostic_log_limit())
}

fn safe_report_path(path: &Path) -> String {
    // Paths can carry odd bytes from archives, so route them through the same terminal guard
    safe_report_value(&path.display().to_string())
}

fn collect_theme_path_warnings(config: &Config) -> Vec<String> {
    let config_root = Path::new("$XDG_CONFIG_HOME/unixnotis");
    let normalized_root = normalize_lexical_path(config_root);
    let theme_paths = match config.resolve_theme_paths_from(config_root) {
        Ok(paths) => paths,
        Err(err) => {
            return vec![format!(
                "unable to resolve theme paths from config.toml: {err}"
            )];
        }
    };

    let mut warnings = Vec::new();
    for (slot, path) in [
        ("base_css", &theme_paths.base_css),
        ("panel_css", &theme_paths.panel_css),
        ("popup_css", &theme_paths.popup_css),
        ("widgets_css", &theme_paths.widgets_css),
        ("media_css", &theme_paths.media_css),
    ] {
        let normalized_path = normalize_lexical_path(path);
        if !normalized_path.starts_with(&normalized_root) {
            warnings.push(format!(
                "theme.{slot} points outside the config root: {}",
                path.display()
            ));
        }
    }
    warnings
}
