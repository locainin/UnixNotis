use super::super::PopupConfig;

#[test]
fn popup_defaults_limit_the_visible_stack_to_three_notifications() {
    let popup = PopupConfig::default();

    assert_eq!(popup.max_visible, 3);
}

#[test]
fn popup_defaults_include_edge_clearance_for_card_shadow() {
    let popup = PopupConfig::default();

    // Both left and right margins accommodate the card box-shadow (~17px blur)
    // so the shadow is not clipped at the work-area boundary regardless of anchor.
    assert_eq!(popup.margin.left, 18);
    assert_eq!(popup.margin.right, 18);
    assert_eq!(popup.margin.top, 14);
    assert_eq!(popup.margin.bottom, 14);
}

#[test]
fn popup_config_without_hover_setting_enables_pause_for_upgrade_compatibility() {
    let popup: PopupConfig = toml::from_str("width = 420")
        .expect("popup config written before hover pause should parse");

    assert!(popup.pause_on_hover);
    assert_eq!(popup.width, 420);
}
