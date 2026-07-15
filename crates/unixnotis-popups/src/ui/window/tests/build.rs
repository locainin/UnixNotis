use super::resolve_popup_width;
use unixnotis_core::Config;

#[test]
fn popup_width_uses_config_value_without_monitor_geometry() {
    let mut config = Config::default();
    config.popups.width = 412;

    assert_eq!(resolve_popup_width(&config, None), 412);
}

#[test]
fn popup_width_never_returns_zero_without_monitor_geometry() {
    let mut config = Config::default();
    config.popups.width = 0;

    assert_eq!(resolve_popup_width(&config, None), 1);
}
