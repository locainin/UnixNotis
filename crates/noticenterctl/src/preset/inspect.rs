//! Preset inspect flow for printing bundle contents and command references
//!
//! Inspect is read-only and is meant to answer two questions quickly:
//! what files are inside the preset, and what command-bearing config fields it carries

use anyhow::{Context, Result};
use std::path::Path;
use unixnotis_core::Config;

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
    out.push_str(&format!("preset: {}\n", bundle.manifest.bundle_name));
    out.push_str(&format!(
        "format version: {}\n",
        bundle.manifest.format_version
    ));
    out.push_str(&format!("exported at: {}\n", bundle.manifest.exported_at));
    out.push_str(&format!("tool version: {}\n", bundle.manifest.tool_version));
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
            Ok(contents) => match toml::from_str::<Config>(contents) {
                Ok(config) => {
                    let commands = collect_command_references_from_config(&config);
                    out.push_str(&format!("command refs: {}\n", commands.len()));
                    if commands.is_empty() {
                        out.push_str("  none\n");
                    } else {
                        for command in commands {
                            out.push_str(&format!("  - {} = {}\n", command.slot, command.command));
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
                                warning.slot, warning.command
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
                                leak.slot, leak.command
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
                            out.push_str(&format!("  - {warning}\n"));
                        }
                    }
                }
                Err(err) => {
                    out.push_str(&format!("command refs: unavailable ({err})\n"));
                }
            },
            Err(err) => {
                out.push_str(&format!("command refs: unavailable ({err})\n"));
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
                warning.css_file.display(),
                warning.asset_ref,
                warning.reason
            ));
        }
    }

    out.push_str("file list:\n");
    for file in &bundle.manifest.files {
        out.push_str(&format!("  - {}\n", file.path));
    }
    Ok(out)
}

fn yes_no(value: bool) -> &'static str {
    // Small helper keeps inspect output predictable and grep-friendly
    if value {
        "yes"
    } else {
        "no"
    }
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

#[cfg(test)]
mod tests {
    use super::inspect_preset_at;
    use crate::preset::export::export_preset_from;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(name: &str) -> Self {
            // Unique temp roots keep inspect tests independent from export and import tests
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock moved backwards")
                .as_nanos();
            let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "unixnotis-preset-inspect-{}-{}-{}",
                name, stamp, serial
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn write(&self, relative_path: &str, contents: &str) {
            // Helper keeps the test body focused on the reported output
            let path = self.path.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent dirs");
            }
            fs::write(path, contents).expect("write file");
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn inspect_lists_bundle_metadata_and_commands() {
        // Inspect should expose the command-bearing parts of the shared config
        let root = TempDirGuard::new("report");
        root.write(
            "config.toml",
            "[theme]\nbase_css = \"base.css\"\n[widgets.volume]\nget_cmd = \"wpctl get-volume @DEFAULT_AUDIO_SINK@\"\n",
        );
        root.write("base.css", ".a { color: red; }");
        let bundle_path = root.path.join("demo.unixnotis");
        export_preset_from(&root.path, &bundle_path, &[], false).expect("export");

        let report = inspect_preset_at(&bundle_path).expect("inspect");

        assert!(report.contains("preset: demo"));
        assert!(report.contains("widgets.volume.get_cmd"));
        assert!(report.contains("command path warnings:"));
        assert!(report.contains("host-specific command paths:"));
        assert!(report.contains("file list:"));
        assert!(report.contains("config.toml"));
    }
}
