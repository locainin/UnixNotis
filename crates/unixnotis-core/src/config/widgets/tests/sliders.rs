#![allow(
    clippy::float_cmp,
    reason = "TOML parsing preserves these exactly representable slider values"
)]

use crate::{NumericParseMode, SliderWidgetConfig, WidgetsConfig};

#[test]
fn default_slider_widgets_keep_stock_commands() {
    let widgets = WidgetsConfig::default();

    assert!(widgets.volume.enabled);
    assert_eq!(widgets.volume.label, "Volume");
    assert_eq!(widgets.volume.get_cmd, SliderWidgetConfig::WPCTL_GET);
    assert_eq!(widgets.volume.set_cmd, SliderWidgetConfig::WPCTL_SET);
    assert_eq!(
        widgets.volume.toggle_cmd.as_deref(),
        Some(SliderWidgetConfig::WPCTL_TOGGLE)
    );
    assert_eq!(widgets.volume.watch_cmd, None);
    assert_eq!(widgets.volume.segments, 10);
    assert!(widgets.volume.show_sublabels);
    assert_eq!(widgets.volume.sublabel_min, "MUTE");
    assert_eq!(widgets.volume.sublabel_max, "MAX");

    assert!(widgets.brightness.enabled);
    assert_eq!(widgets.brightness.label, "Brightness");
    assert_eq!(widgets.brightness.get_cmd, "brightnessctl -m");
    assert_eq!(widgets.brightness.set_cmd, "brightnessctl s {value}%");
    assert_eq!(widgets.brightness.watch_cmd, None);
    assert_eq!(widgets.brightness.segments, 10);
    assert!(widgets.brightness.show_sublabels);
    assert_eq!(widgets.brightness.sublabel_min, "MIN");
    assert_eq!(widgets.brightness.sublabel_max, "MAX");
}

#[test]
fn partial_slider_uses_current_decoration_defaults() {
    let slider: SliderWidgetConfig = toml::from_str(
        r#"
        enabled = true
        label = "Volume"
        "#,
    )
    .expect("partial slider should parse");

    assert_eq!(slider.segments, 10);
    assert!(slider.show_sublabels);
    assert_eq!(slider.sublabel_min, "MUTE");
    assert_eq!(slider.sublabel_max, "MAX");
}

#[test]
fn custom_slider_config_parses_numeric_bounds_and_labels() {
    let slider: SliderWidgetConfig = toml::from_str(
        r#"
        enabled = true
        label = "Mic"
        icon = "audio-input-microphone-symbolic"
        icon_muted = "microphone-disabled-symbolic"
        get_cmd = "scripts/mic get"
        set_cmd = "scripts/mic set {value}"
        toggle_cmd = "scripts/mic toggle"
        watch_cmd = "scripts/mic watch"
        min = -12.5
        max = 12.5
        step = 0.5
        show_value = false
        segments = 8
        show_sublabels = true
        sublabel_min = "quiet"
        sublabel_max = "loud"
        parse_mode = "ratio"
        "#,
    )
    .expect("custom slider should parse");

    assert!(slider.enabled);
    assert_eq!(slider.label, "Mic");
    assert_eq!(
        slider.icon_muted.as_deref(),
        Some("microphone-disabled-symbolic")
    );
    assert_eq!(slider.toggle_cmd.as_deref(), Some("scripts/mic toggle"));
    assert_eq!(slider.watch_cmd.as_deref(), Some("scripts/mic watch"));
    assert_eq!(slider.min, -12.5);
    assert_eq!(slider.max, 12.5);
    assert_eq!(slider.step, 0.5);
    assert!(!slider.show_value);
    assert_eq!(slider.segments, 8);
    assert!(slider.show_sublabels);
    assert_eq!(slider.sublabel_min, "quiet");
    assert_eq!(slider.sublabel_max, "loud");
    assert_eq!(slider.parse_mode, NumericParseMode::Ratio);
}

#[test]
fn numeric_parse_modes_accept_only_known_kebab_case_values() {
    let percent: NumericParseMode = toml::from_str("mode = \"percent\"")
        .map(|wrapper: NumericParseModeWrapper| wrapper.mode)
        .expect("percent mode should parse");
    let ratio: NumericParseMode = toml::from_str("mode = \"ratio\"")
        .map(|wrapper: NumericParseModeWrapper| wrapper.mode)
        .expect("ratio mode should parse");

    assert_eq!(percent, NumericParseMode::Percent);
    assert_eq!(ratio, NumericParseMode::Ratio);
    assert!(toml::from_str::<NumericParseModeWrapper>("mode = \"raw\"").is_err());
}

#[derive(serde::Deserialize)]
struct NumericParseModeWrapper {
    mode: NumericParseMode,
}
