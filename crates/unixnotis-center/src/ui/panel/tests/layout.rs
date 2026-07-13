use super::{height_from_percent, resolve_panel_width};
use unixnotis_core::Config;

#[test]
fn height_from_percent_scales_usable_height() {
    assert_eq!(height_from_percent(1000, 84), 840);
    assert_eq!(height_from_percent(701, 84), 589);
}

#[test]
fn repeated_width_resolution_uses_config_instead_of_previous_allocation() {
    let mut config = Config::default();
    config.panel.width = 380;

    for _ in 0..8 {
        assert_eq!(resolve_panel_width(&config, None), 380);
    }

    config.panel.width = 512;
    assert_eq!(resolve_panel_width(&config, None), 512);
    config.panel.width = 380;
    assert_eq!(resolve_panel_width(&config, None), 380);
}

#[test]
fn height_from_percent_keeps_a_positive_floor() {
    assert_eq!(height_from_percent(1, 1), 1);
    assert_eq!(height_from_percent(40, 1), 1);
}
