use super::super::super::super::config_widgets::{CardWidgetConfig, StatWidgetConfig};
use super::super::*;
use crate::{Config, PanelConfig, PopupConfig};

#[test]
fn sanitize_clamps_refresh_intervals_and_preserves_ordering() {
    // Fast and slow refresh loops should stay bounded and ordered
    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 1;
    config.widgets.refresh_interval_slow_ms = 50;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, MIN_REFRESH_MS);
    assert_eq!(config.widgets.refresh_interval_slow_ms, MIN_REFRESH_MS);

    let mut config = Config::default();
    config.widgets.refresh_interval_ms = MAX_REFRESH_MS + 10;
    config.widgets.refresh_interval_slow_ms = MAX_REFRESH_SLOW_MS + 10;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, MAX_REFRESH_MS);
    assert_eq!(config.widgets.refresh_interval_slow_ms, MAX_REFRESH_SLOW_MS);

    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 0;
    config.widgets.refresh_interval_slow_ms = 0;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 0);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 0);

    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 0;
    config.widgets.refresh_interval_slow_ms = 200;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 0);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 200);

    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 200;
    config.widgets.refresh_interval_slow_ms = 0;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 200);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 0);

    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 200;
    config.widgets.refresh_interval_slow_ms = 100;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 200);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 200);
}

#[test]
fn sanitize_clamps_panel_and_popup_sizes() {
    // Broken panel and popup sizes should fall back into safe geometry
    let mut config = Config::default();
    config.panel.width = 0;
    config.panel.height = -8;
    config.panel.height_override = Some(-4);
    config.popups.width = -10;
    config.popups.spacing = -3;
    sanitize_config(&mut config);
    assert_eq!(config.panel.width, PanelConfig::default().width);
    assert_eq!(config.panel.height, PanelConfig::default().height);
    assert_eq!(config.panel.height_override, None);
    assert_eq!(config.popups.width, PopupConfig::default().width);
    assert_eq!(config.popups.spacing, 0);

    let mut config = Config::default();
    config.panel.width = MAX_PANEL_WIDTH + 25;
    config.panel.height = MAX_PANEL_HEIGHT_PERCENT + 40;
    config.panel.height_override = Some(MAX_PANEL_HEIGHT + 40);
    config.popups.width = MAX_POPUP_WIDTH + 30;
    config.popups.spacing = MAX_SPACING + 12;
    sanitize_config(&mut config);
    assert_eq!(config.panel.width, MAX_PANEL_WIDTH);
    assert_eq!(config.panel.height, MAX_PANEL_HEIGHT_PERCENT);
    assert_eq!(config.panel.height_override, Some(MAX_PANEL_HEIGHT));
    assert_eq!(config.popups.width, MAX_POPUP_WIDTH);
    assert_eq!(config.popups.spacing, MAX_SPACING);
}

#[test]
fn sanitize_clamps_history_limits() {
    // History limits should respect both hard caps
    let mut config = Config::default();
    config.history.max_active = MAX_HISTORY_ACTIVE + 1_000;
    config.history.max_entries = MAX_HISTORY_ENTRIES + 10_000;
    sanitize_config(&mut config);
    assert_eq!(config.history.max_active, MAX_HISTORY_ACTIVE);
    assert_eq!(config.history.max_entries, MAX_HISTORY_ENTRIES);
}

#[test]
fn sanitize_keeps_active_history_within_total_history() {
    // Active rows should never outgrow the total history budget
    let mut config = Config::default();
    config.history.max_active = 12;
    config.history.max_entries = 1;

    sanitize_config(&mut config);

    assert_eq!(config.history.max_entries, 1);
    assert_eq!(config.history.max_active, 1);
}

#[test]
fn sanitize_clamps_margins_and_card_heights() {
    // Margin and min-height clamping should cover both stats and cards
    let mut config = Config::default();
    while config.widgets.stats.len() < 2 {
        config.widgets.stats.push(StatWidgetConfig::default());
    }
    while config.widgets.cards.len() < 2 {
        config.widgets.cards.push(CardWidgetConfig::default());
    }

    config.popups.margin.top = -4;
    config.popups.margin.right = MAX_MARGIN + 3;
    config.popups.margin.bottom = -9;
    config.popups.margin.left = MAX_MARGIN + 8;
    config.panel.margin.top = MAX_MARGIN + 6;
    config.panel.margin.right = -5;
    config.panel.margin.bottom = MAX_MARGIN + 4;
    config.panel.margin.left = -7;

    config.widgets.stats[0].min_height = -1;
    config.widgets.stats[1].min_height = MAX_CARD_HEIGHT + 11;
    config.widgets.cards[0].min_height = -2;
    config.widgets.cards[1].min_height = MAX_CARD_HEIGHT + 21;
    sanitize_config(&mut config);

    assert_eq!(config.popups.margin.top, 0);
    assert_eq!(config.popups.margin.right, MAX_MARGIN);
    assert_eq!(config.popups.margin.bottom, 0);
    assert_eq!(config.popups.margin.left, MAX_MARGIN);
    assert_eq!(config.panel.margin.top, MAX_MARGIN);
    assert_eq!(config.panel.margin.right, 0);
    assert_eq!(config.panel.margin.bottom, MAX_MARGIN);
    assert_eq!(config.panel.margin.left, 0);

    assert_eq!(config.widgets.stats[0].min_height, 0);
    assert_eq!(config.widgets.stats[1].min_height, MAX_CARD_HEIGHT);
    assert_eq!(config.widgets.cards[0].min_height, 0);
    assert_eq!(config.widgets.cards[1].min_height, MAX_CARD_HEIGHT);
}

#[test]
fn widget_toggle_tooltips_parse_cleanly() {
    let mut config: Config = toml::from_str(
        r#"
        [widgets]
        toggle_tooltips = true
        "#,
    )
    .expect("config should parse");
    sanitize_config(&mut config);

    assert!(config.widgets.toggle_tooltips);
}
