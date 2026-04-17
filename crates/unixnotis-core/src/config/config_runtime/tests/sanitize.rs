use super::super::super::config_widgets::{CardWidgetConfig, StatWidgetConfig, WidgetPluginConfig};
use super::*;

#[test]
fn media_aliases_load_blacklist_and_whitelist() {
    // Legacy names still need to map to the same media lists
    let mut config: Config = toml::from_str(
        r#"
        [media]
        whitelist = ["Spotify"]
        blacklist = ["Playerctld"]
        "#,
    )
    .expect("config should parse");
    sanitize_config(&mut config);

    assert_eq!(config.media.allowlist, vec!["spotify".to_string()]);
    assert_eq!(config.media.denylist, vec!["playerctld".to_string()]);
}

#[test]
fn sanitize_clamps_refresh_intervals_and_preserves_ordering() {
    // Ensure the fast interval clamps to bounds and slow interval does not undercut fast.
    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 1;
    config.widgets.refresh_interval_slow_ms = 50;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, MIN_REFRESH_MS);
    assert_eq!(config.widgets.refresh_interval_slow_ms, MIN_REFRESH_MS);

    // Ensure upper bounds are enforced for both fast and slow refresh intervals.
    let mut config = Config::default();
    config.widgets.refresh_interval_ms = MAX_REFRESH_MS + 10;
    config.widgets.refresh_interval_slow_ms = MAX_REFRESH_SLOW_MS + 10;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, MAX_REFRESH_MS);
    assert_eq!(config.widgets.refresh_interval_slow_ms, MAX_REFRESH_SLOW_MS);

    // Preserve disabled intervals at 0 to avoid re-enabling polling.
    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 0;
    config.widgets.refresh_interval_slow_ms = 0;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 0);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 0);

    // Allow slow refresh to remain enabled when fast refresh is disabled.
    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 0;
    config.widgets.refresh_interval_slow_ms = 200;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 0);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 200);

    // Preserve slow-disabled state when fast refresh is enabled.
    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 200;
    config.widgets.refresh_interval_slow_ms = 0;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 200);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 0);

    // Enforce slow >= fast when both are enabled.
    let mut config = Config::default();
    config.widgets.refresh_interval_ms = 200;
    config.widgets.refresh_interval_slow_ms = 100;
    sanitize_config(&mut config);
    assert_eq!(config.widgets.refresh_interval_ms, 200);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 200);
}

#[test]
fn sanitize_clamps_panel_and_popup_sizes() {
    // Validate default sizing is restored when invalid or negative inputs are provided
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

    // Validate size limits are enforced for oversized values
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
    // History stays within the cap
    let mut config = Config::default();
    config.history.max_active = MAX_HISTORY_ACTIVE + 1_000;
    config.history.max_entries = MAX_HISTORY_ENTRIES + 10_000;
    sanitize_config(&mut config);
    assert_eq!(config.history.max_active, MAX_HISTORY_ACTIVE);
    assert_eq!(config.history.max_entries, MAX_HISTORY_ENTRIES);
}

#[test]
fn sanitize_keeps_active_history_within_total_history() {
    let mut config = Config::default();
    config.history.max_active = 12;
    config.history.max_entries = 1;

    sanitize_config(&mut config);

    assert_eq!(config.history.max_entries, 1);
    assert_eq!(config.history.max_active, 1);
}

#[test]
fn sanitize_clamps_margins_and_card_heights() {
    // Ensure the config has enough widgets for min-height coverage.
    let mut config = Config::default();
    while config.widgets.stats.len() < 2 {
        config.widgets.stats.push(StatWidgetConfig::default());
    }
    while config.widgets.cards.len() < 2 {
        config.widgets.cards.push(CardWidgetConfig::default());
    }

    // Validate margins clamp to non-negative values and maximum bounds.
    config.popups.margin.top = -4;
    config.popups.margin.right = MAX_MARGIN + 3;
    config.popups.margin.bottom = -9;
    config.popups.margin.left = MAX_MARGIN + 8;
    config.panel.margin.top = MAX_MARGIN + 6;
    config.panel.margin.right = -5;
    config.panel.margin.bottom = MAX_MARGIN + 4;
    config.panel.margin.left = -7;

    // Validate stat/card min-height values clamp to the allowable range.
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
fn sanitize_normalizes_media_tokens() {
    // Normalize case, drop empty entries, and remove repeats from token lists.
    let mut config = Config::default();
    config.media.allowlist = vec![
        "Spotify".to_string(),
        " ".to_string(),
        "spotify".to_string(),
    ];
    config.media.denylist = vec!["Playerctld".to_string(), "playerctld".to_string()];
    config.media.browser_tokens =
        vec!["FireFox".to_string(), "".to_string(), "firefox".to_string()];
    sanitize_config(&mut config);

    assert_eq!(config.media.allowlist, vec!["spotify".to_string()]);
    assert_eq!(config.media.denylist, vec!["playerctld".to_string()]);
    assert_eq!(config.media.browser_tokens, vec!["firefox".to_string()]);
    assert_eq!(
        config.media.remote_art_policy,
        super::super::super::MediaRemoteArtPolicy::NativeOnly
    );
}

#[test]
fn media_layout_and_metadata_flags_parse_cleanly() {
    let mut config: Config = toml::from_str(
        r#"
        [media]
        layout = "showcase"
        show_source = false
        show_position = false
        show_artist = false
        show_title = false
        show_art = false
        show_controls = false
        show_navigation = false
        title_fallback = "empty"
        position_format = "current"
        art_position = "top"
        controls_position = "side"
        navigation_position = "external"
        art_size_px = 88
        text_width_floor_px = 192
        content_spacing_px = 12
        control_spacing_px = 10
        navigation_spacing_px = 14
        [media.source_aliases]
        spotify = "Spotify Player"
        "#,
    )
    .expect("config should parse");
    sanitize_config(&mut config);

    assert_eq!(
        config.media.layout,
        super::super::super::MediaLayout::Showcase
    );
    assert!(!config.media.show_source);
    assert!(!config.media.show_position);
    assert!(!config.media.show_artist);
    assert!(!config.media.show_title);
    assert!(!config.media.show_art);
    assert!(!config.media.show_controls);
    assert!(!config.media.show_navigation);
    assert_eq!(
        config.media.title_fallback,
        super::super::super::MediaTitleFallback::Empty
    );
    assert_eq!(
        config.media.position_format,
        super::super::super::MediaPositionFormat::Current
    );
    assert_eq!(
        config.media.art_position,
        super::super::super::MediaArtPosition::Top
    );
    assert_eq!(
        config.media.controls_position,
        super::super::super::MediaControlsPosition::Side
    );
    assert_eq!(
        config.media.navigation_position,
        super::super::super::MediaNavigationPosition::External
    );
    assert_eq!(config.media.art_size_px, 88);
    assert_eq!(config.media.text_width_floor_px, 192);
    assert_eq!(config.media.content_spacing_px, 12);
    assert_eq!(config.media.control_spacing_px, 10);
    assert_eq!(config.media.navigation_spacing_px, 14);
    assert_eq!(
        config
            .media
            .source_aliases
            .get("spotify")
            .map(String::as_str),
        Some("Spotify Player")
    );
}

#[test]
fn sanitize_clamps_media_geometry_and_aliases() {
    let mut config = Config::default();
    config.media.title_char_limit = 0;
    config.media.art_size_px = -10;
    config.media.text_width_floor_px = -1;
    config.media.content_spacing_px = -4;
    config.media.control_spacing_px = 999;
    config.media.navigation_spacing_px = 999;
    config.media.card_height_px = Some(-2);
    config
        .media
        .source_aliases
        .insert(" Spotify ".to_string(), " Spotify App ".to_string());
    config
        .media
        .source_aliases
        .insert(" ".to_string(), "ignored".to_string());
    config
        .media
        .source_aliases
        .insert("vlc".to_string(), " ".to_string());

    sanitize_config(&mut config);

    assert_eq!(config.media.title_char_limit, 1);
    assert_eq!(
        config.media.art_size_px,
        super::super::super::MediaConfig::default().art_size_px
    );
    assert_eq!(
        config.media.text_width_floor_px,
        super::super::super::MediaConfig::default().text_width_floor_px
    );
    assert_eq!(config.media.content_spacing_px, 0);
    assert_eq!(config.media.control_spacing_px, MAX_SPACING);
    assert_eq!(config.media.navigation_spacing_px, MAX_SPACING);
    assert_eq!(config.media.card_height_px, None);
    assert_eq!(config.media.source_aliases.len(), 1);
    assert_eq!(
        config
            .media
            .source_aliases
            .get("spotify")
            .map(String::as_str),
        Some("Spotify App")
    );
}

#[test]
fn sanitize_keeps_first_alias_when_lowercase_keys_collide() {
    let mut config = Config::default();
    config
        .media
        .source_aliases
        .insert("Spotify".to_string(), "Spotify Desktop".to_string());
    config
        .media
        .source_aliases
        .insert("spotify".to_string(), "Spotify Web".to_string());

    sanitize_config(&mut config);

    assert_eq!(config.media.source_aliases.len(), 1);
    assert_eq!(
        config
            .media
            .source_aliases
            .get("spotify")
            .map(String::as_str),
        Some("Spotify Desktop")
    );
}

#[test]
fn media_allowlist_and_whitelist_together_fail_to_parse() {
    let err = toml::from_str::<Config>(
        r#"
        [media]
        allowlist = ["spotify"]
        whitelist = ["firefox"]
        "#,
    )
    .expect_err("duplicate logical media field should fail");

    assert!(
        err.to_string().to_lowercase().contains("duplicate"),
        "error should mention duplicate field semantics: {err}"
    );
}

#[test]
fn media_denylist_and_blacklist_together_fail_to_parse() {
    let err = toml::from_str::<Config>(
        r#"
        [media]
        denylist = ["playerctld"]
        blacklist = ["spotify"]
        "#,
    )
    .expect_err("duplicate logical media field should fail");

    assert!(
        err.to_string().to_lowercase().contains("duplicate"),
        "error should mention duplicate field semantics: {err}"
    );
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

#[test]
fn sanitize_clamps_alpha_and_theme_limits() {
    // Validate alpha values clamp to [0, 1] and fall back on non-finite inputs.
    let mut config = Config::default();
    let theme_defaults = ThemeConfig::default();
    config.theme.surface_alpha = -0.25;
    config.theme.surface_strong_alpha = 1.25;
    config.theme.card_alpha = f32::NAN;
    config.theme.shadow_soft_alpha = f32::INFINITY;
    config.theme.shadow_strong_alpha = -0.5;
    config.theme.border_width = MAX_BORDER_WIDTH + 2;
    config.theme.card_radius = MAX_CARD_RADIUS + 3;
    sanitize_config(&mut config);

    assert_eq!(config.theme.surface_alpha, 0.0);
    assert_eq!(config.theme.surface_strong_alpha, 1.0);
    assert!(
        (config.theme.card_alpha - theme_defaults.card_alpha).abs() < f32::EPSILON,
        "card alpha fallback should match theme default"
    );
    assert!(
        (config.theme.shadow_soft_alpha - theme_defaults.shadow_soft_alpha).abs() < f32::EPSILON,
        "shadow soft alpha fallback should match theme default"
    );
    assert_eq!(config.theme.shadow_strong_alpha, 0.0);
    assert_eq!(config.theme.border_width, MAX_BORDER_WIDTH);
    assert_eq!(config.theme.card_radius, MAX_CARD_RADIUS);
}

#[test]
fn sanitize_clamps_alpha_without_defaults() {
    // Validate finite alpha values clamp without forcing theme defaults.
    let mut config = Config::default();
    config.theme.surface_alpha = 1.5;
    config.theme.surface_strong_alpha = -0.2;
    config.theme.card_alpha = 0.2;
    config.theme.shadow_soft_alpha = 2.0;
    config.theme.shadow_strong_alpha = -1.0;
    sanitize_config(&mut config);

    assert_eq!(config.theme.surface_alpha, 1.0);
    assert_eq!(config.theme.surface_strong_alpha, 0.0);
    assert_eq!(config.theme.card_alpha, 0.2);
    assert_eq!(config.theme.shadow_soft_alpha, 1.0);
    assert_eq!(config.theme.shadow_strong_alpha, 0.0);
}

#[test]
fn sanitize_widget_plugin_clamps_bounds_and_trim_command() {
    // Trim and clamp the plugin config
    let mut config = Config::default();
    config.widgets.stats[0].plugin = Some(WidgetPluginConfig {
        command: "  script arg  ".to_string(),
        timeout_ms: MAX_PLUGIN_TIMEOUT_MS + 1,
        max_output_bytes: MAX_PLUGIN_OUTPUT_BYTES + 10,
        ..WidgetPluginConfig::default()
    });
    sanitize_config(&mut config);

    let plugin = config.widgets.stats[0]
        .plugin
        .as_ref()
        .expect("plugin should remain enabled");
    assert_eq!(plugin.command, "script arg");
    assert_eq!(plugin.timeout_ms, MAX_PLUGIN_TIMEOUT_MS);
    assert_eq!(plugin.max_output_bytes, MAX_PLUGIN_OUTPUT_BYTES);
}

#[test]
fn sanitize_widget_plugin_rejects_shell_meta_commands() {
    // Shell syntax is rejected
    let mut config = Config::default();
    config.widgets.cards[0].plugin = Some(WidgetPluginConfig {
        command: "sh -c 'echo pwned | cat'".to_string(),
        ..WidgetPluginConfig::default()
    });
    sanitize_config(&mut config);
    assert!(config.widgets.cards[0].plugin.is_none());
}
