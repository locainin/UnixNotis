//! Toggle icon resolution helpers
//!
//! Keeps icon fallback policy focused and testable away from widget lifecycle code

use unixnotis_core::ToggleWidgetConfig;

/// Resolves the icon name for a toggle while keeping fallback semantics
/// inside the same domain as the toggle kind
pub(super) fn resolve_toggle_icon_name(config: &ToggleWidgetConfig) -> String {
    // Empty and whitespace-only values are treated the same
    let requested = config.icon.trim();

    // Empty configured icons still get a stable generic symbol
    if requested.is_empty() {
        return "applications-system-symbolic".to_string();
    }

    // Display can be unavailable during early startup windows
    let Some(display) = gtk::gdk::Display::default() else {
        return requested.to_string();
    };
    let theme = gtk::IconTheme::for_display(&display);
    // Kind is inferred once and reused across airplane overrides and fallbacks
    let kind = infer_toggle_icon_kind(config, requested);

    // Prefer flightmode aliases for airplane when available.
    // Several icon themes provide better airplane visuals under these names.
    if kind == ToggleIconKind::Airplane {
        if let Some(preferred) = preferred_airplane_icon_name(requested, &theme) {
            return preferred;
        }
    }

    // Keep configured icon when the active theme has it
    // This preserves user and default config intent, including airplane styling
    if theme.has_icon(requested) {
        return requested.to_string();
    }

    // Some themes expose only symbolic or only non-symbolic aliases
    if let Some(alias) = resolve_symbolic_alias(requested, &theme) {
        return alias;
    }

    // Infer semantic kind from kind/label/requested name so custom configs still map well
    for fallback in toggle_icon_fallbacks(kind) {
        if theme.has_icon(fallback) {
            return fallback.to_string();
        }
    }

    // Always attempt safe generic symbols before returning a possibly-missing request
    // This prevents red missing-icon placeholders when themes omit airplane aliases
    for fallback in common_icon_fallbacks() {
        if theme.has_icon(fallback) {
            return fallback.to_string();
        }
    }

    // Keep original request as the absolute final fallback
    requested.to_string()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToggleIconKind {
    Wifi,
    Bluetooth,
    Airplane,
    Night,
    Unknown,
}

fn resolve_symbolic_alias(requested: &str, theme: &gtk::IconTheme) -> Option<String> {
    // Preserve symbolic icon intent for toggle styling consistency
    if requested.ends_with("-symbolic") {
        return None;
    }

    // Try adding -symbolic for themes that only expose symbolic names
    {
        let symbolic = format!("{requested}-symbolic");
        if theme.has_icon(&symbolic) {
            return Some(symbolic);
        }
    }
    None
}

fn infer_toggle_icon_kind(config: &ToggleWidgetConfig, requested: &str) -> ToggleIconKind {
    // Normalize all signal sources so matching is case-insensitive
    let kind = config
        .kind
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let label = config.label.to_ascii_lowercase();
    let requested = requested.to_ascii_lowercase();
    // Sources include kind, label, and icon to support older custom configs
    let sources = [kind.as_str(), label.as_str(), requested.as_str()];

    // Bluetooth is matched first to avoid wireless overlap ambiguity
    if matches_any(&sources, &["bluetooth", "bluez", "network-bluetooth"]) {
        return ToggleIconKind::Bluetooth;
    }
    // Airplane includes multiple aliases for legacy and custom config names
    if matches_any(
        &sources,
        &[
            "airplane",
            "flightmode",
            "flight-mode",
            "flight_mode",
            "network-flightmode",
        ],
    ) {
        return ToggleIconKind::Airplane;
    }
    // Backend aliases only affect icon choice; commands still come from config
    if matches_any(
        &sources,
        &["night", "sunset", "gamma", "wlsunset", "hyprsunset"],
    ) {
        return ToggleIconKind::Night;
    }
    // Wifi falls back last because many icon names include generic network terms
    if matches_any(
        &sources,
        &["wifi", "wi-fi", "wireless", "wlan", "network-wireless"],
    ) {
        return ToggleIconKind::Wifi;
    }
    ToggleIconKind::Unknown
}

fn matches_any(sources: &[&str], needles: &[&str]) -> bool {
    // Substring matching handles values like network-flightmode-on and flight_mode
    sources.iter().copied().any(|source| {
        needles
            .iter()
            .copied()
            .any(|needle| source.contains(needle))
    })
}

fn preferred_airplane_icon_name(requested: &str, theme: &gtk::IconTheme) -> Option<String> {
    let requested = requested.to_ascii_lowercase();
    if !requested.contains("airplane") {
        // Limit this override to airplane-labeled requests only
        return None;
    }

    for candidate in [
        // Prefer symbolic flightmode aliases to avoid dark hardcoded airplane glyphs
        "network-flightmode-on-symbolic",
        "network-flightmode-off-symbolic",
        // Keep non-symbolic fallbacks for themes without symbolic variants
        "network-flightmode-on",
        "flightmode-on",
    ] {
        if theme.has_icon(candidate) {
            // First match keeps icon selection stable across refreshes
            return Some(candidate.to_string());
        }
    }
    None
}

fn toggle_icon_fallbacks(kind: ToggleIconKind) -> &'static [&'static str] {
    // Keep candidate ordering deterministic so icon selection remains predictable
    match kind {
        ToggleIconKind::Wifi => &[
            // Prefer explicit signal and connected names before generic fallbacks
            "network-wireless-signal-excellent-symbolic",
            "network-wireless-signal-good-symbolic",
            "network-wireless-symbolic",
            "network-wireless-connected-symbolic",
            "network-wireless-signal-excellent",
            "network-wireless",
            "network-workgroup-symbolic",
            "network-workgroup",
        ],
        ToggleIconKind::Bluetooth => &[
            // Cover Adwaita, Breeze, and mixed custom themes
            "bluetooth-active-symbolic",
            "bluetooth-symbolic",
            "network-bluetooth-symbolic",
            "network-wireless-bluetooth-symbolic",
            "network-bluetooth",
            "preferences-system-bluetooth-activated-symbolic",
            "preferences-system-bluetooth-symbolic",
            "preferences-bluetooth-symbolic",
            "preferences-system-bluetooth",
            "bluetooth",
        ],
        ToggleIconKind::Airplane => &[
            // Prefer symbolic names first so tinting matches other toggle icons
            "airplane-mode-symbolic",
            "airplane-mode-disabled-symbolic",
            "network-flightmode-on-symbolic",
            "network-flightmode-off-symbolic",
            "route-transit-airplane-symbolic",
            "xsi-airplane-symbolic",
            "network-wireless-offline-symbolic",
            "xsi-network-wireless-offline-symbolic",
            // Fallback to non-symbolic flightmode names used by Breeze-like themes
            "network-flightmode-on",
            "network-flightmode-off",
            "flightmode-on",
            "flightmode-off",
            "transport-mode-flight",
            "airplane-mode",
            "airplane",
            // Last semantic symbol before generic cross-kind fallback
            "network-wireless-disabled-symbolic",
        ],
        ToggleIconKind::Night => &[
            // Keep night icon semantics close to moon/brightness symbols
            "weather-clear-night-symbolic",
            "weather-clear-night",
            "display-brightness-symbolic",
            "display-brightness",
            "preferences-system-symbolic",
            "preferences-system",
        ],
        ToggleIconKind::Unknown => &[],
    }
}

fn common_icon_fallbacks() -> &'static [&'static str] {
    &[
        // Generic symbols prevent missing-icon placeholders when kind-specific names are absent
        "applications-system-symbolic",
        "preferences-system-symbolic",
        "network-wireless-symbolic",
    ]
}

#[cfg(test)]
mod tests {
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
}
