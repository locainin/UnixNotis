use super::*;

#[test]
fn custom_volume_without_watch_stays_config_owned() {
    let mut volume = SliderWidgetConfig {
        enabled: true,
        label: "Volume".to_string(),
        icon: "audio-volume-high-symbolic".to_string(),
        icon_muted: None,
        get_cmd: "custom-volume-get".to_string(),
        set_cmd: "custom-volume-set {value}".to_string(),
        toggle_cmd: None,
        watch_cmd: None,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        show_value: true,
        segments: 0,
        show_sublabels: false,
        sublabel_min: String::new(),
        sublabel_max: String::new(),
        parse_mode: NumericParseMode::Auto,
    };

    apply_volume_backend(&mut volume);

    assert!(volume.watch_cmd.is_none());
}

#[test]
fn legacy_brightness_watch_is_removed() {
    let mut brightness = SliderWidgetConfig {
        enabled: true,
        label: "Brightness".to_string(),
        icon: "display-brightness-symbolic".to_string(),
        icon_muted: None,
        get_cmd: "brightnessctl -m".to_string(),
        set_cmd: "brightnessctl s {value}%".to_string(),
        toggle_cmd: None,
        watch_cmd: Some("brightnessctl -w".to_string()),
        min: 1.0,
        max: 100.0,
        step: 1.0,
        show_value: true,
        segments: 0,
        show_sublabels: false,
        sublabel_min: String::new(),
        sublabel_max: String::new(),
        parse_mode: NumericParseMode::Auto,
    };

    apply_brightness_backend(&mut brightness);

    assert!(brightness.watch_cmd.is_none());
}
