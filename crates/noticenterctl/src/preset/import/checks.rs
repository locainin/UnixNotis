//! Import validation helpers for hostile preset content
//!
//! These checks run before import writes anything to disk so
//! crafted bundles fail early instead of escaping through later setup steps

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use unixnotis_core::{Config, ThemePaths};

use super::super::command_rules::{
    validate_command_paths_in_config_bytes, validate_config_command_paths_stay_in_root,
};
use super::super::pathing::normalize_lexical_path;

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

#[cfg(test)]
mod tests {
    use super::{
        validate_imported_command_paths_stay_in_root, validate_imported_theme_paths_stay_in_root,
    };
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
}
