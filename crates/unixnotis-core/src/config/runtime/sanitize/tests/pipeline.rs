use super::super::super::super::widgets::{CardWidgetConfig, StatWidgetConfig};
use super::*;
use crate::{
    Config, DndMenuChoice, DndMenuTrigger, PanelActionConfig, PanelActionId, PanelConfig,
    PanelSection, PanelWidgetSection, PopupConfig, ToggleLayout, WidgetPluginConfig,
    CURRENT_CONFIG_VERSION, MAX_CARD_WIDGETS, MAX_STAT_WIDGETS, MAX_TOGGLE_WIDGETS,
    MAX_TOTAL_WIDGETS,
};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

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

    let mut config = Config::default();
    config.widgets.refresh_interval_ms = MIN_REFRESH_MS;
    config.widgets.refresh_interval_slow_ms = MIN_REFRESH_MS - 1;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, MIN_REFRESH_MS);
    assert_eq!(config.widgets.refresh_interval_slow_ms, MIN_REFRESH_MS);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x554e_4958_4e4f_5449),
        ..ProptestConfig::default()
    })]

    #[test]
    fn parsed_config_reports_are_deterministic_bounded_and_current(
        fast in 0_u64..200_000,
        slow in 0_u64..250_000,
        max_active in 0_usize..10_000,
        max_entries in 0_usize..20_000,
        width in -10_000_i32..10_000,
        label in "[a-zA-Z0-9 _-]{0,256}",
    ) {
        // Backend detection reads PATH, so paired parses must exclude env-mutating tests
        let _environment = crate::test_support::test_env_lock();
        let input = format!(
            "config_version = {CURRENT_CONFIG_VERSION}\n[panel]\nwidth = {width}\n[history]\nmax_active = {max_active}\nmax_entries = {max_entries}\n[widgets]\nrefresh_interval_ms = {fast}\nrefresh_interval_slow_ms = {slow}\nquick_actions_label = {label:?}\n"
        );
        let first = Config::parse_with_report(&input).expect("generated config should parse");
        let second = Config::parse_with_report(&input).expect("same generated config should parse");

        prop_assert_eq!(first.config.config_version, CURRENT_CONFIG_VERSION);
        prop_assert!(first.config.widgets.refresh_interval_ms <= MAX_REFRESH_MS);
        prop_assert!(first.config.widgets.refresh_interval_slow_ms <= MAX_REFRESH_SLOW_MS);
        prop_assert!(first.config.history.max_active <= MAX_HISTORY_ACTIVE);
        prop_assert!(first.config.history.max_entries <= MAX_HISTORY_ENTRIES);
        // Table ordering can differ because user aliases are stored in hash maps
        let first_value = toml::Value::try_from(&first.config).expect("serialize first config");
        let second_value = toml::Value::try_from(&second.config).expect("serialize second config");
        prop_assert_eq!(first_value, second_value);
        prop_assert_eq!(first.diagnostics, second.diagnostics);
    }

    #[test]
    fn arbitrary_config_text_never_panics_and_accepted_results_stay_bounded(
        characters in prop::collection::vec(any::<char>(), 0..=2_048),
    ) {
        let _environment = crate::test_support::test_env_lock();
        let input = characters.into_iter().collect::<String>();
        let outcome = std::panic::catch_unwind(|| Config::parse_with_report(&input));

        prop_assert!(outcome.is_ok());
        if let Ok(report) = outcome.expect("panic outcome checked above") {
            prop_assert_eq!(report.config.config_version, CURRENT_CONFIG_VERSION);
            prop_assert!(report.config.widgets.toggles.len() <= MAX_TOGGLE_WIDGETS);
            prop_assert!(report.config.widgets.stats.len() <= MAX_STAT_WIDGETS);
            prop_assert!(report.config.widgets.cards.len() <= MAX_CARD_WIDGETS);
            prop_assert!(
                report.config.widgets.toggles.len()
                    + report.config.widgets.stats.len()
                    + report.config.widgets.cards.len()
                    <= MAX_TOTAL_WIDGETS
            );
            prop_assert!(report.config.history.max_active <= MAX_HISTORY_ACTIVE);
            prop_assert!(report.config.history.max_entries <= MAX_HISTORY_ENTRIES);
            prop_assert!(report
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.message.chars().count() <= 256));
        }
    }

    #[test]
    fn unsupported_plugin_versions_never_survive_sanitization(
        api_version in any::<u32>().prop_filter(
            "generated version must be unsupported",
            |version| *version != WidgetPluginConfig::API_VERSION_V1,
        ),
        command in "[a-zA-Z0-9_ ./-]{1,256}",
    ) {
        let mut config = Config::default();
        config.widgets.stats[0].plugin = Some(WidgetPluginConfig {
            api_version,
            command,
            ..WidgetPluginConfig::default()
        });

        sanitize_config(&mut config);

        prop_assert!(config.widgets.stats[0].plugin.is_none());
    }

    #[test]
    fn generated_widget_counts_always_fit_per_kind_and_total_limits(
        toggle_count in 0_usize..100,
        stat_count in 0_usize..100,
        card_count in 0_usize..100,
    ) {
        let defaults = Config::default();
        let mut config = defaults.clone();
        config.widgets.toggles = vec![defaults.widgets.toggles[0].clone(); toggle_count];
        config.widgets.stats = vec![defaults.widgets.stats[0].clone(); stat_count];
        config.widgets.cards = vec![defaults.widgets.cards[0].clone(); card_count];

        sanitize_config(&mut config);

        prop_assert!(config.widgets.toggles.len() <= MAX_TOGGLE_WIDGETS);
        prop_assert!(config.widgets.stats.len() <= MAX_STAT_WIDGETS);
        prop_assert!(config.widgets.cards.len() <= MAX_CARD_WIDGETS);
        prop_assert!(
            config.widgets.toggles.len()
                + config.widgets.stats.len()
                + config.widgets.cards.len()
                <= MAX_TOTAL_WIDGETS
        );
    }
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
fn sanitize_bounds_every_custom_notification_metadata_string() {
    let mut config = Config::default();
    let oversized = "界".repeat(160);
    config.panel.notification_metadata.critical_label = oversized.clone();
    config.panel.notification_metadata.low_label = oversized.clone();
    config.panel.notification_metadata.normal_label = oversized.clone();
    config.panel.notification_metadata.relative_now = oversized.clone();
    config.panel.notification_metadata.relative_minutes = oversized.clone();
    config.panel.notification_metadata.relative_hours = oversized.clone();
    config.panel.notification_metadata.relative_days = oversized.clone();
    config.panel.notification_metadata.transient_label = oversized.clone();
    config.panel.notification_metadata.live_label = oversized.clone();
    config.panel.notification_metadata.history_label = oversized.clone();
    config.panel.notification_metadata.action_count_one = oversized.clone();
    config.panel.notification_metadata.action_count_many = oversized;

    sanitize_config(&mut config);

    for text in [
        &config.panel.notification_metadata.critical_label,
        &config.panel.notification_metadata.low_label,
        &config.panel.notification_metadata.normal_label,
        &config.panel.notification_metadata.relative_now,
        &config.panel.notification_metadata.relative_minutes,
        &config.panel.notification_metadata.relative_hours,
        &config.panel.notification_metadata.relative_days,
        &config.panel.notification_metadata.transient_label,
        &config.panel.notification_metadata.live_label,
        &config.panel.notification_metadata.history_label,
        &config.panel.notification_metadata.action_count_one,
        &config.panel.notification_metadata.action_count_many,
    ] {
        assert_eq!(text.chars().count(), 128);
    }
}

#[test]
fn sanitize_preserves_optional_panel_labels_and_repairs_widget_order() {
    let mut config = Config::default();
    config.panel.title = " ".to_string();
    config.panel.search_placeholder.clear();
    config.panel.search_magnifier_icon = "x".repeat(256);
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
    assert_eq!(config.panel.search_magnifier_icon.chars().count(), 128);
    assert!(config.panel.quick_actions_label.is_empty());
    assert!(config.panel.system_status_label.is_empty());
    assert!(config.panel.recent_notifications_label.is_empty());
    assert_eq!(config.panel.clear_label, PanelConfig::default().clear_label);
    assert!(!config.panel.action_row_visible);
    assert_eq!(config.panel.section_order[0], PanelSection::Notifications);
    assert_eq!(config.panel.section_order.len(), 2);
    assert_eq!(
        config.panel.section_order,
        vec![PanelSection::Notifications, PanelSection::Widgets]
    );
    assert_eq!(config.panel.widget_order[0], PanelWidgetSection::Stats);
    assert_eq!(config.panel.widget_order.len(), 5);
    assert_eq!(
        config.panel.widget_order,
        vec![
            PanelWidgetSection::Stats,
            PanelWidgetSection::Media,
            PanelWidgetSection::Sliders,
            PanelWidgetSection::Toggles,
            PanelWidgetSection::Cards,
        ]
    );
    assert_eq!(config.panel.action_order[0], PanelActionId::Search);
    assert_eq!(config.panel.action_order.len(), 4);
    assert_eq!(
        config.panel.action_order,
        vec![
            PanelActionId::Search,
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
        ]
    );
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
fn sanitize_preserves_explicit_close_action_order() {
    let mut config = Config::default();
    config.panel.action_order = vec![
        PanelActionId::Close,
        PanelActionId::Search,
        PanelActionId::Close,
    ];

    sanitize_config(&mut config);

    assert_eq!(
        config.panel.action_order,
        vec![
            PanelActionId::Close,
            PanelActionId::Search,
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
        ]
    );
}

#[test]
fn sanitize_dnd_menu_deduplicates_triggers_and_bounds_choices() {
    let mut config = Config::default();
    config.panel.dnd_menu_triggers = vec![
        DndMenuTrigger::RightClick,
        DndMenuTrigger::Keyboard,
        DndMenuTrigger::RightClick,
    ];
    config.panel.dnd_menu_choices = vec![
        DndMenuChoice::Duration {
            label: String::new(),
            minutes: 0,
        },
        DndMenuChoice::Duration {
            label: "Year".to_string(),
            minutes: u32::MAX,
        },
        DndMenuChoice::Tomorrow {
            label: "Next day".to_string(),
            hour: u8::MAX,
            minute: u8::MAX,
        },
    ];

    sanitize_config(&mut config);

    assert_eq!(
        config.panel.dnd_menu_triggers,
        vec![DndMenuTrigger::RightClick, DndMenuTrigger::Keyboard]
    );
    assert_eq!(config.panel.dnd_menu_choices.len(), 2);
    assert!(matches!(
        config.panel.dnd_menu_choices[0],
        DndMenuChoice::Duration {
            minutes: 525_600,
            ..
        }
    ));
    assert!(matches!(
        config.panel.dnd_menu_choices[1],
        DndMenuChoice::Tomorrow {
            hour: 23,
            minute: 59,
            ..
        }
    ));
}

#[test]
fn sanitize_preserves_an_explicitly_disabled_dnd_menu() {
    let mut config = Config::default();
    config.panel.dnd_menu_triggers.clear();
    config.panel.dnd_menu_choices.clear();

    sanitize_config(&mut config);

    assert!(config.panel.dnd_menu_triggers.is_empty());
    assert!(config.panel.dnd_menu_choices.is_empty());
}

#[test]
fn default_panel_section_labels_name_the_visible_widget_groups() {
    let config = PanelConfig::default();

    assert_eq!(config.quick_actions_label, "Quick settings");
    assert_eq!(config.system_status_label, "System health");
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
fn sanitize_keeps_active_limit_independent_from_history_retention() {
    let mut config = Config::default();
    config.history.max_active = 12;
    config.history.max_entries = 0;

    sanitize_config(&mut config);

    assert_eq!(config.history.max_entries, 0);
    assert_eq!(config.history.max_active, 12);
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
