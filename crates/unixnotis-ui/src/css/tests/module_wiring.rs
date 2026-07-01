use unixnotis_core::ThemeConfig;

#[test]
fn css_root_can_reach_renamed_override_module() {
    // This catches stale #[path] wiring after moving css_overrides_tests into css/tests
    let theme = ThemeConfig {
        border_width: 4,
        card_radius: 14,
        ..ThemeConfig::default()
    };

    let panel = super::overrides::build_panel_overrides(&theme);
    let widgets = super::overrides::build_widgets_overrides(&theme);
    let popups = super::overrides::build_popup_overrides(&theme);

    assert!(panel.contains("border-width: 4px;"));
    assert!(widgets.contains("border-radius: 14px;"));
    assert!(popups.contains("border-width: 4px;"));
}
