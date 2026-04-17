use super::super::*;
use crate::Config;

#[test]
fn media_aliases_load_blacklist_and_whitelist() {
    // Legacy names should still land in the current allow and deny lists
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
fn sanitize_normalizes_media_tokens() {
    // Token lists should be lowercase, non-empty, and duplicate-free
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
        super::super::super::super::MediaRemoteArtPolicy::NativeOnly
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
        super::super::super::super::MediaLayout::Showcase
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
        super::super::super::super::MediaTitleFallback::Empty
    );
    assert_eq!(
        config.media.position_format,
        super::super::super::super::MediaPositionFormat::Current
    );
    assert_eq!(
        config.media.art_position,
        super::super::super::super::MediaArtPosition::Top
    );
    assert_eq!(
        config.media.controls_position,
        super::super::super::super::MediaControlsPosition::Side
    );
    assert_eq!(
        config.media.navigation_position,
        super::super::super::super::MediaNavigationPosition::External
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
        super::super::super::super::MediaConfig::default().art_size_px
    );
    assert_eq!(
        config.media.text_width_floor_px,
        super::super::super::super::MediaConfig::default().text_width_floor_px
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
