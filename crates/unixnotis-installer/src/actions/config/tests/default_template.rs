use super::super::provision::render_default_config_toml;
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
