use super::{infer_toggle_icon_kind, toggle_icon_fallbacks, ToggleIconKind};
use unixnotis_core::ToggleWidgetConfig;

fn test_toggle(kind: Option<&str>, label: &str, icon: &str) -> ToggleWidgetConfig {
    ToggleWidgetConfig {
        enabled: true,
        kind: kind.map(str::to_string),
        label: label.to_string(),
        icon: icon.to_string(),
        state_cmd: None,
        toggle_cmd: None,
        on_cmd: None,
        off_cmd: None,
        watch_cmd: None,
    }
}

#[test]
fn infer_toggle_icon_kind_uses_kind_label_and_icon_tokens() {
    let bluetooth = test_toggle(None, "Bluetooth", "network-wireless-symbolic");
    assert_eq!(
        infer_toggle_icon_kind(&bluetooth, bluetooth.icon.as_str()),
        ToggleIconKind::Bluetooth
    );

    let airplane = test_toggle(
        Some("flight_mode"),
        "Travel",
        "applications-system-symbolic",
    );
    assert_eq!(
        infer_toggle_icon_kind(&airplane, airplane.icon.as_str()),
        ToggleIconKind::Airplane
    );

    let wifi = test_toggle(None, "Wi-Fi", "network-wireless-signal-excellent-symbolic");
    assert_eq!(
        infer_toggle_icon_kind(&wifi, wifi.icon.as_str()),
        ToggleIconKind::Wifi
    );

    let unknown = test_toggle(None, "Custom", "applications-system-symbolic");
    assert_eq!(
        infer_toggle_icon_kind(&unknown, unknown.icon.as_str()),
        ToggleIconKind::Unknown
    );
}

#[test]
fn bluetooth_fallbacks_include_breeze_family_names() {
    let fallbacks = toggle_icon_fallbacks(ToggleIconKind::Bluetooth);
    assert!(fallbacks.contains(&"network-bluetooth-symbolic"));
    assert!(fallbacks.contains(&"preferences-system-bluetooth-symbolic"));
}

#[test]
fn airplane_fallbacks_prefer_symbolic_family() {
    let fallbacks = toggle_icon_fallbacks(ToggleIconKind::Airplane);
    assert_eq!(fallbacks.first().copied(), Some("airplane-mode-symbolic"));
    assert!(fallbacks.contains(&"route-transit-airplane-symbolic"));
    assert!(fallbacks.contains(&"network-flightmode-on"));
    assert!(fallbacks.contains(&"flightmode-on"));
}
