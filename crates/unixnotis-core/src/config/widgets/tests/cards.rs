use crate::{CardLayout, CardWidgetConfig, WidgetPluginConfig, WidgetsConfig};

#[test]
fn default_card_widgets_keep_builtin_identity_and_layout() {
    let widgets = WidgetsConfig::default();
    let calendar = &widgets.cards[0];
    let weather = &widgets.cards[1];

    assert!(!calendar.enabled);
    assert_eq!(calendar.kind.as_deref(), Some("calendar"));
    assert_eq!(calendar.layout, CardLayout::Default);
    assert_eq!(calendar.title, "Calendar");
    assert_eq!(calendar.icon.as_deref(), Some("x-office-calendar-symbolic"));
    assert_eq!(calendar.icon_asset, None);
    assert_eq!(calendar.min_height, 180);
    assert_eq!(calendar.cmd, None);

    assert!(!weather.enabled);
    assert_eq!(weather.kind.as_deref(), Some("weather"));
    assert_eq!(weather.title, "Weather");
    assert_eq!(weather.subtitle.as_deref(), Some("No data"));
    assert_eq!(weather.icon.as_deref(), Some("weather-clear-symbolic"));
    assert_eq!(weather.icon_asset, None);
    assert_eq!(weather.min_height, 160);
}

#[test]
fn blank_card_default_is_disabled_placeholder() {
    let card = CardWidgetConfig::default();

    assert!(!card.enabled);
    assert_eq!(card.kind, None);
    assert_eq!(card.layout, CardLayout::Default);
    assert_eq!(card.title, "Card");
    assert_eq!(card.subtitle, None);
    assert_eq!(card.icon, None);
    assert_eq!(card.icon_asset, None);
    assert_eq!(card.cmd, None);
    assert_eq!(card.plugin, None);
    assert_eq!(card.min_height, 120);
    assert!(!card.monospace);
    assert_eq!(card.carousel_dots, 0);
    assert!(!card.carousel_arrows);
}

#[test]
fn custom_card_layout_and_carousel_options_parse() {
    let card: CardWidgetConfig = toml::from_str(
        r#"
        enabled = true
        kind = "hero"
        layout = "image-row"
        title = "Now"
        subtitle = "Live"
        icon = "image-x-generic-symbolic"
        icon_asset = "assets/card.webp"
        cmd = "scripts/card"
        min_height = 220
        monospace = true
        carousel_dots = 5
        carousel_arrows = true

        [plugin]
        api_version = 1
        command = "scripts/card-plugin"
        timeout_ms = 3000
        max_output_bytes = 4096
        "#,
    )
    .expect("card should parse");

    assert!(card.enabled);
    assert_eq!(card.kind.as_deref(), Some("hero"));
    assert_eq!(card.layout, CardLayout::ImageRow);
    assert_eq!(card.title, "Now");
    assert_eq!(card.subtitle.as_deref(), Some("Live"));
    assert_eq!(card.icon.as_deref(), Some("image-x-generic-symbolic"));
    assert_eq!(card.icon_asset.as_deref(), Some("assets/card.webp"));
    assert_eq!(card.cmd.as_deref(), Some("scripts/card"));
    assert_eq!(card.min_height, 220);
    assert!(card.monospace);
    assert_eq!(card.carousel_dots, 5);
    assert!(card.carousel_arrows);
    assert_eq!(
        card.plugin,
        Some(WidgetPluginConfig {
            api_version: 1,
            command: "scripts/card-plugin".to_string(),
            timeout_ms: 3000,
            max_output_bytes: 4096,
        })
    );
}

#[test]
fn card_layout_parses_banner_and_rejects_unknown_values() {
    #[derive(serde::Deserialize)]
    struct LayoutFixture {
        layout: CardLayout,
    }

    let parsed: LayoutFixture = toml::from_str("layout = \"banner\"").expect("banner should parse");

    assert_eq!(parsed.layout, CardLayout::Banner);
    assert!(toml::from_str::<LayoutFixture>("layout = \"poster\"").is_err());
}
