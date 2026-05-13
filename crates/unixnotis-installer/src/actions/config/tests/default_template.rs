use std::fs;
use std::path::PathBuf;

use super::super::provision::{render_default_config_toml, write_default_scripts};
use unixnotis_core::Config;

#[test]
fn default_config_template_documents_panel_height_modes() {
    let config_toml = render_default_config_toml(&Config::default()).expect("render config");
    assert!(config_toml.contains("# Vertical size as a percent of usable monitor height"));
    assert!(config_toml.contains("height = 84"));
    assert!(config_toml.contains("# height_override = 1487"));
    assert!(!config_toml
        .lines()
        .any(|line| line.trim() == "height_override = 1487"));
}

#[test]
fn default_config_template_uses_shipped_night_scripts() {
    let config_toml = render_default_config_toml(&Config::default()).expect("render config");
    let night_block = config_toml
        .split("[[widgets.toggles]]")
        .find(|block| block.contains("kind = \"night\""))
        .expect("night toggle block");

    // The default config stays functional while backend logic lives in editable scripts
    assert!(night_block.contains("enabled = true"));
    assert!(night_block.contains("state_cmd = \"scripts/unixnotis-blue-light-state\""));
    assert!(night_block.contains("on_cmd = \"scripts/unixnotis-blue-light-on\""));
    assert!(night_block.contains("off_cmd = \"scripts/unixnotis-blue-light-off\""));
    assert!(!night_block.contains("gammastep"));
    assert!(!night_block.contains("hyprsunset"));
    assert!(!night_block.contains("wlsunset"));
}

#[test]
fn default_blue_light_scripts_are_shipped_with_config() {
    let script_paths = unixnotis_core::DEFAULT_SCRIPTS
        .iter()
        .map(|script| script.relative_path)
        .collect::<Vec<_>>();

    assert!(script_paths.contains(&"scripts/unixnotis-blue-light-state"));
    assert!(script_paths.contains(&"scripts/unixnotis-blue-light-lib"));
    assert!(script_paths.contains(&"scripts/unixnotis-blue-light-on"));
    assert!(script_paths.contains(&"scripts/unixnotis-blue-light-off"));
    assert!(unixnotis_core::DEFAULT_SCRIPTS
        .iter()
        .all(|script| script.contents.starts_with("#!/bin/sh\n")));
}

#[test]
fn write_default_scripts_creates_executable_helpers() {
    let root = PathBuf::from("target").join(format!(
        "unixnotis-default-script-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);

    write_default_scripts(&root).expect("write default scripts");

    for script in unixnotis_core::DEFAULT_SCRIPTS {
        let path = root.join(script.relative_path);
        let contents = fs::read_to_string(&path).expect("read default script");
        assert_eq!(contents, script.contents);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&path)
                .expect("script metadata")
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "script should be executable: {path:?}");
        }
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_config_template_does_not_emit_known_invalid_watch_commands() {
    let config_toml = render_default_config_toml(&Config::default()).expect("render config");

    // Invalid watchers must not be written and then silently cleaned at runtime
    // Generated config should match the behavior the UI actually uses
    assert!(!config_toml.contains("brightnessctl -w"));
    assert!(!config_toml.contains("wpctl subscribe"));
}
