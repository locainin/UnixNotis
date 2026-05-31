use super::super::super::super::widget_config::{CardWidgetConfig, StatWidgetConfig};
use super::*;
use crate::{
    Config, PanelActionConfig, PanelActionId, PanelConfig, PanelSection, PanelWidgetSection,
    PopupConfig, ToggleLayout,
};

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
fn sanitize_preserves_optional_panel_labels_and_repairs_widget_order() {
    let mut config = Config::default();
    config.panel.title = " ".to_string();
    config.panel.search_placeholder.clear();
    config.panel.quick_actions_label.clear();
    config.panel.system_status_label.clear();
    config.panel.recent_notifications_label.clear();
    config.panel.clear_label.clear();
    config.panel.action_row_visible = false;
    config.panel.section_order = vec![PanelSection::Notifications, PanelSection::Notifications];
    config.panel.widget_order = vec![PanelWidgetSection::Stats, PanelWidgetSection::Stats];
    config.panel.action_order = vec![PanelActionId::Search, PanelActionId::Search];
    config.panel.search_action.icon.clear();
    config.panel.search_action.tooltip.clear();
    config.panel.close_action = PanelActionConfig::default();

    sanitize_config(&mut config);

    assert_eq!(config.panel.title, PanelConfig::default().title);
    assert!(config.panel.search_placeholder.is_empty());
    assert!(config.panel.quick_actions_label.is_empty());
    assert!(config.panel.system_status_label.is_empty());
    assert!(config.panel.recent_notifications_label.is_empty());
    assert_eq!(config.panel.clear_label, PanelConfig::default().clear_label);
    assert!(!config.panel.action_row_visible);
    assert_eq!(config.panel.section_order[0], PanelSection::Notifications);
    assert_eq!(config.panel.section_order.len(), 2);
    assert_eq!(config.panel.widget_order[0], PanelWidgetSection::Stats);
    assert_eq!(config.panel.widget_order.len(), 5);
    assert_eq!(config.panel.action_order[0], PanelActionId::Search);
    assert_eq!(config.panel.action_order.len(), 4);
    assert_eq!(
        config.panel.search_action.icon,
        PanelActionConfig::search().icon
    );
    assert_eq!(
        config.panel.search_action.tooltip,
        PanelActionConfig::search().tooltip
    );
    assert_eq!(config.panel.close_action, PanelActionConfig::close());
}

#[test]
fn default_panel_section_labels_do_not_force_widget_headings() {
    let config = PanelConfig::default();

    // Widget section headings are config-owned and hidden unless explicitly set
    assert!(config.quick_actions_label.is_empty());
    assert!(config.system_status_label.is_empty());
}

#[test]
fn sanitize_preserves_icon_only_action_blocks_with_default_chrome() {
    let mut config = Config::default();
    config.panel.clear_action = PanelActionConfig {
        icon_only: true,
        ..PanelActionConfig::default()
    };

    sanitize_config(&mut config);

    assert!(config.panel.clear_action.icon_only);
    assert_eq!(
        config.panel.clear_action.icon,
        PanelActionConfig::clear().icon
    );
    assert_eq!(
        config.panel.clear_action.tooltip,
        PanelActionConfig::clear().tooltip
    );
    assert!(
        config.panel.clear_action.label.is_empty(),
        "icon-only actions may intentionally hide text labels"
    );
}

#[test]
fn sanitize_clamps_widget_grid_columns() {
    let mut config = Config::default();
    config.widgets.toggle_columns = 0;
    config.widgets.stat_columns = MAX_WIDGET_COLUMNS + 10;
    config.widgets.card_columns = 0;

    sanitize_config(&mut config);

    assert_eq!(
        config.widgets.toggle_columns,
        crate::WidgetsConfig::default().toggle_columns
    );
    assert_eq!(config.widgets.stat_columns, MAX_WIDGET_COLUMNS);
    assert_eq!(
        config.widgets.card_columns,
        crate::WidgetsConfig::default().card_columns
    );
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
        toggle_layout = "vertical"
        toggle_columns = 3
        stat_columns = 4
        card_columns = 1

        [[widgets.toggles]]
        enabled = true
        label = "Custom Action"
        icon = "applications-system-symbolic"
        toggle_cmd = "scripts/custom-action"
        "#,
    )
    .expect("config should parse");
    sanitize_config(&mut config);

    assert!(config.widgets.toggle_tooltips);
    assert_eq!(config.widgets.toggle_layout, ToggleLayout::Vertical);
    assert_eq!(config.widgets.toggle_columns, 3);
    assert_eq!(config.widgets.stat_columns, 4);
    assert_eq!(config.widgets.card_columns, 1);
    assert_eq!(
        config.widgets.toggles[0].toggle_cmd.as_deref(),
        Some("scripts/custom-action")
    );
}
