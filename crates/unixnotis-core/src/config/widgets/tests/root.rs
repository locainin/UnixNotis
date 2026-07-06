use crate::{ToggleLayout, WidgetsConfig};

#[test]
fn default_widgets_keep_expected_grid_shape() {
    let widgets = WidgetsConfig::default();

    // These counts define the visible stock control-center sections
    assert_eq!(widgets.toggle_layout, ToggleLayout::Horizontal);
    assert_eq!(widgets.toggle_columns, 4);
    assert_eq!(widgets.stat_columns, 2);
    assert_eq!(widgets.card_columns, 2);
    assert_eq!(widgets.toggles.len(), 4);
    assert_eq!(widgets.stats.len(), 3);
    assert_eq!(widgets.cards.len(), 2);
}

#[test]
fn partial_widgets_toml_keeps_unspecified_stock_defaults() {
    let widgets: WidgetsConfig = toml::from_str(
        r#"
        toggle_layout = "vertical"
        toggle_columns = 3
        refresh_interval_ms = 2500
        "#,
    )
    .expect("partial widgets config should parse");

    assert_eq!(widgets.toggle_layout, ToggleLayout::Vertical);
    assert_eq!(widgets.toggle_columns, 3);
    assert_eq!(widgets.refresh_interval_ms, 2500);
    assert_eq!(widgets.stat_columns, WidgetsConfig::default().stat_columns);
    assert_eq!(widgets.cards.len(), WidgetsConfig::default().cards.len());
    assert_eq!(widgets.volume.label, "Volume");
}

#[test]
fn empty_widget_arrays_replace_stock_sections() {
    let widgets: WidgetsConfig = toml::from_str(
        r#"
        toggles = []
        stats = []
        cards = []
        "#,
    )
    .expect("empty widget arrays should parse");

    assert!(widgets.toggles.is_empty());
    assert!(widgets.stats.is_empty());
    assert!(widgets.cards.is_empty());
    assert!(widgets.volume.enabled);
    assert!(widgets.brightness.enabled);
}
