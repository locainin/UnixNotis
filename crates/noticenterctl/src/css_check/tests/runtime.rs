use std::path::{Path, PathBuf};

use unixnotis_core::{Config, PANEL_RUNTIME_WIDTH_MIN};

use super::super::runtime::{display_config_path, panel_width_floor_warning};

#[test]
fn display_config_path_matches_css_display_style_when_config_is_inside_root() {
    let config_dir = PathBuf::from("/tmp/unixnotis-config");
    let config_path = config_dir.join("config.toml");

    assert_eq!(
        display_config_path(&config_dir, "$CONFIG/unixnotis", &config_path),
        "$CONFIG/unixnotis/config.toml"
    );
}

#[test]
fn display_config_path_keeps_external_config_paths_literal() {
    let config_dir = PathBuf::from("/tmp/unixnotis-config");

    assert_eq!(
        display_config_path(
            &config_dir,
            "$CONFIG/unixnotis",
            Path::new("/tmp/other/config.toml")
        ),
        "/tmp/other/config.toml"
    );
}

#[test]
fn panel_width_floor_warning_only_reports_widths_below_runtime_floor() {
    let mut config = Config::default();
    config.panel.width = PANEL_RUNTIME_WIDTH_MIN - 1;
    assert!(panel_width_floor_warning(&config)
        .expect("floor warning")
        .contains("runtime floor"));

    config.panel.width = PANEL_RUNTIME_WIDTH_MIN;
    assert!(panel_width_floor_warning(&config).is_none());

    config.panel.width = PANEL_RUNTIME_WIDTH_MIN + 1;
    assert!(panel_width_floor_warning(&config).is_none());
}
