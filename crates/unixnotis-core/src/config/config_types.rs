//! Configuration types and defaults for UnixNotis.
//!
//! Keeps schema definitions in one place for easier auditing.

use serde::{Deserialize, Serialize};

/// Top-level configuration loaded from config.toml.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub popups: PopupConfig,
    pub panel: PanelConfig,
    pub history: HistoryConfig,
    pub media: MediaConfig,
    pub widgets: WidgetsConfig,
    pub sound: SoundConfig,
    pub theme: ThemeConfig,
    pub rules: Vec<RuleConfig>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub dnd_default: bool,
    pub log_level: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PopupConfig {
    pub anchor: Anchor,
    pub margin: Margins,
    pub width: i32,
    pub spacing: i32,
    pub max_visible: usize,
    pub default_timeout_ms: u64,
    pub critical_timeout_ms: Option<u64>,
    pub allow_click_through: bool,
    pub output: Option<String>,
}

impl Default for PopupConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopRight,
            margin: Margins::default(),
            width: 360,
            spacing: 12,
            max_visible: 4,
            default_timeout_ms: 5000,
            critical_timeout_ms: None,
            allow_click_through: false,
            output: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PanelConfig {
    pub anchor: Anchor,
    pub margin: Margins,
    pub width: i32,
    pub height: i32,
    pub keyboard_interactivity: PanelKeyboardInteractivity,
    pub output: Option<String>,
    /// Hide the panel when focus leaves the window.
    pub close_on_blur: bool,
    /// Close the panel when a different window becomes active (Hyprland only).
    pub close_on_click_outside: bool,
    /// Respect compositor reserved work area when computing height (Hyprland only).
    pub respect_work_area: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::Right,
            margin: Margins {
                top: 54,
                right: 6,
                bottom: 6,
                left: 6,
            },
            width: 420,
            height: 0,
            keyboard_interactivity: PanelKeyboardInteractivity::OnDemand,
            output: None,
            close_on_blur: false,
            close_on_click_outside: true,
            respect_work_area: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub max_entries: usize,
    pub max_active: usize,
    pub transient_to_history: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 200,
            max_active: 500,
            transient_to_history: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct MediaConfig {
    /// Enable the media widget in the notification center.
    pub enabled: bool,
    /// Include web browser media players.
    pub include_browsers: bool,
    /// Characters allowed before marquee scrolling begins.
    pub title_char_limit: usize,
    /// Allowlist of player identifiers or bus names (case-insensitive substrings).
    pub allowlist: Vec<String>,
    /// Denylist of player identifiers or bus names (case-insensitive substrings).
    pub denylist: Vec<String>,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            include_browsers: true,
            title_char_limit: 32,
            allowlist: Vec::new(),
            denylist: vec!["playerctld".to_string()],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WidgetsConfig {
    pub volume: SliderWidgetConfig,
    pub brightness: SliderWidgetConfig,
    pub toggles: Vec<ToggleWidgetConfig>,
    pub stats: Vec<StatWidgetConfig>,
    pub cards: Vec<CardWidgetConfig>,
    pub refresh_interval_ms: u64,
    pub refresh_interval_slow_ms: u64,
}

impl Default for WidgetsConfig {
    fn default() -> Self {
        Self {
            volume: SliderWidgetConfig::default_volume(),
            brightness: SliderWidgetConfig::default_brightness(),
            toggles: vec![
                ToggleWidgetConfig::default_wifi(),
                ToggleWidgetConfig::default_bluetooth(),
                ToggleWidgetConfig::default_airplane(),
                ToggleWidgetConfig::default_night(),
            ],
            stats: vec![
                StatWidgetConfig::default_cpu(),
                StatWidgetConfig::default_memory(),
                StatWidgetConfig::default_battery(),
            ],
            cards: vec![
                CardWidgetConfig::default_calendar(),
                CardWidgetConfig::default_weather(),
            ],
            refresh_interval_ms: 1000,
            refresh_interval_slow_ms: 3000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SliderWidgetConfig {
    pub enabled: bool,
    pub label: String,
    pub icon: String,
    pub icon_muted: Option<String>,
    pub get_cmd: String,
    pub set_cmd: String,
    pub toggle_cmd: Option<String>,
    pub watch_cmd: Option<String>,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Controls how numeric command output is interpreted for slider values.
    pub parse_mode: NumericParseMode,
}

impl SliderWidgetConfig {
    // wpctl (PipeWire/WirePlumber CLI) volume commands.
    // These are fast and avoid a shell wrapper when available.
    pub(super) const WPCTL_GET: &'static str = "wpctl get-volume @DEFAULT_AUDIO_SINK@";
    pub(super) const WPCTL_SET: &'static str = "wpctl set-volume @DEFAULT_AUDIO_SINK@ {value}%";
    pub(super) const WPCTL_TOGGLE: &'static str = "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle";

    // pactl (PulseAudio / pipewire-pulse) volume commands.
    // PACTL_GET relies on shell sequencing to capture volume + mute in one call.
    pub(super) const PACTL_GET: &'static str =
        "pactl get-sink-volume @DEFAULT_SINK@; pactl get-sink-mute @DEFAULT_SINK@";
    pub(super) const PACTL_SET: &'static str = "pactl set-sink-volume @DEFAULT_SINK@ {value}%";
    pub(super) const PACTL_TOGGLE: &'static str = "pactl set-sink-mute @DEFAULT_SINK@ toggle";

    // Long-running watcher for audio changes; emits events and stays open.
    // The UI/daemon can listen to this and refresh on demand instead of polling.
    pub(super) const PACTL_WATCH: &'static str = "pactl subscribe";

    fn default_volume() -> Self {
        // Default config for the Volume slider widget.
        // Uses wpctl by default (common on PipeWire setups), with runtime fallback support elsewhere.
        Self {
            enabled: true, // Enabled in the stock config; disable in config to hide.
            label: "Volume".to_string(),
            icon: "audio-volume-high-symbolic".to_string(),
            icon_muted: Some("audio-volume-muted-symbolic".to_string()),

            // Commands are templates; runtime replaces tokens like {value} and default sink placeholders.
            get_cmd: Self::WPCTL_GET.to_string(),
            set_cmd: Self::WPCTL_SET.to_string(),
            toggle_cmd: Some(Self::WPCTL_TOGGLE.to_string()),

            // Watcher is applied at runtime when a supported long-running command is available.
            // Keeping this None in defaults avoids silently configuring a watcher that may not exist.
            watch_cmd: None,

            // Slider range and granularity (UI uses these for adjustment and formatting).
            min: 0.0,
            max: 100.0,
            step: 1.0,
            parse_mode: NumericParseMode::Auto,
        }
    }

    fn default_brightness() -> Self {
        // Default config for the Brightness slider widget.
        // brightnessctl typically supports get/set, but it does not have a universal watch mode.
        Self {
            enabled: true,
            label: "Brightness".to_string(),
            icon: "display-brightness-symbolic".to_string(),
            icon_muted: None,

            // -m outputs machine-readable values; parsing stays stable.
            get_cmd: "brightnessctl -m".to_string(),
            set_cmd: "brightnessctl s {value}%".to_string(),
            toggle_cmd: None,

            // Watch mode is not reliably supported by brightnessctl; leaving this here means
            // spawning may fail and the widget will fall back to polling.
            // Runtime clears invalid watchers, so this value is treated as None.
            watch_cmd: Some("brightnessctl -w".to_string()),

            min: 0.0,
            max: 100.0,
            step: 1.0,
            parse_mode: NumericParseMode::Auto,
        }
    }
}

impl Default for SliderWidgetConfig {
    fn default() -> Self {
        Self::default_volume()
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum NumericParseMode {
    /// Uses heuristic parsing for mixed output formats.
    Auto,
    /// Interprets values as percentages without normalization.
    Percent,
    /// Interprets values as 0.0-1.0 ratios and scales to percent.
    Ratio,
}

impl Default for NumericParseMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ToggleWidgetConfig {
    pub enabled: bool,
    pub label: String,
    pub icon: String,
    pub state_cmd: Option<String>,
    pub on_cmd: Option<String>,
    pub off_cmd: Option<String>,
    pub watch_cmd: Option<String>,
}

impl ToggleWidgetConfig {
    fn default_wifi() -> Self {
        Self {
            enabled: true,
            label: "Wi-Fi".to_string(),
            icon: "network-wireless-signal-excellent-symbolic".to_string(),
            state_cmd: Some("nmcli radio wifi".to_string()),
            on_cmd: Some("nmcli radio wifi on".to_string()),
            off_cmd: Some("nmcli radio wifi off".to_string()),
            watch_cmd: Some("nmcli -t monitor".to_string()),
        }
    }

    fn default_bluetooth() -> Self {
        Self {
            enabled: true,
            label: "Bluetooth".to_string(),
            icon: "bluetooth-active-symbolic".to_string(),
            state_cmd: Some("bluetoothctl show".to_string()),
            on_cmd: Some("bluetoothctl power on".to_string()),
            off_cmd: Some("bluetoothctl power off".to_string()),
            watch_cmd: Some("bluetoothctl --monitor".to_string()),
        }
    }

    fn default_airplane() -> Self {
        Self {
            enabled: true,
            label: "Airplane".to_string(),
            icon: "airplane-mode-symbolic".to_string(),
            state_cmd: Some("rfkill list all | grep -q \"Soft blocked: yes\"".to_string()),
            on_cmd: Some("rfkill block all".to_string()),
            off_cmd: Some("rfkill unblock all".to_string()),
            watch_cmd: Some("udevadm monitor --udev --subsystem-match=rfkill".to_string()),
        }
    }

    fn default_night() -> Self {
        Self {
            enabled: true,
            label: "Night".to_string(),
            icon: "weather-clear-night-symbolic".to_string(),
            state_cmd: None,
            on_cmd: None,
            off_cmd: None,
            watch_cmd: None,
        }
    }
}

impl Default for ToggleWidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            label: "Toggle".to_string(),
            icon: "applications-system-symbolic".to_string(),
            state_cmd: None,
            on_cmd: None,
            off_cmd: None,
            watch_cmd: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct StatWidgetConfig {
    pub enabled: bool,
    pub label: String,
    pub icon: Option<String>,
    pub kind: Option<String>,
    pub cmd: Option<String>,
    pub min_height: i32,
}

impl StatWidgetConfig {
    fn default_cpu() -> Self {
        Self {
            enabled: true,
            label: "CPU".to_string(),
            icon: Some("utilities-system-monitor-symbolic".to_string()),
            kind: None,
            cmd: Some("builtin:cpu".to_string()),
            min_height: 72,
        }
    }

    fn default_memory() -> Self {
        Self {
            enabled: true,
            label: "RAM".to_string(),
            icon: Some("drive-harddisk-symbolic".to_string()),
            kind: None,
            cmd: Some("builtin:memory".to_string()),
            min_height: 72,
        }
    }

    fn default_battery() -> Self {
        Self {
            enabled: true,
            label: "Battery".to_string(),
            icon: Some("battery-full-symbolic".to_string()),
            kind: None,
            cmd: Some("builtin:battery".to_string()),
            min_height: 72,
        }
    }
}

impl Default for StatWidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            label: "Stat".to_string(),
            icon: None,
            kind: None,
            cmd: None,
            min_height: 72,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CardWidgetConfig {
    pub enabled: bool,
    pub kind: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub cmd: Option<String>,
    pub min_height: i32,
    pub monospace: bool,
}

impl CardWidgetConfig {
    fn default_calendar() -> Self {
        Self {
            enabled: true,
            kind: Some("calendar".to_string()),
            title: "Calendar".to_string(),
            subtitle: None,
            icon: Some("x-office-calendar-symbolic".to_string()),
            cmd: None,
            min_height: 180,
            monospace: false,
        }
    }

    fn default_weather() -> Self {
        Self {
            enabled: true,
            kind: Some("weather".to_string()),
            title: "Weather".to_string(),
            subtitle: Some("No data".to_string()),
            icon: Some("weather-clear-symbolic".to_string()),
            cmd: None,
            min_height: 160,
            monospace: false,
        }
    }
}

impl Default for CardWidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: None,
            title: "Card".to_string(),
            subtitle: None,
            icon: None,
            cmd: None,
            min_height: 120,
            monospace: false,
        }
    }
}

/// Notification sound behavior.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SoundConfig {
    /// Enables sound playback when the daemon receives notifications.
    pub enabled: bool,
    /// Default named sound from the freedesktop sound theme.
    pub default_name: Option<String>,
    /// Default sound file path, resolves relative to the UnixNotis config dir.
    pub default_file: Option<String>,
    /// Directory containing custom sound files, resolves relative to config dir.
    pub default_dir: Option<String>,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_name: Some("message-new-instant".to_string()),
            default_file: None,
            default_dir: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ThemeConfig {
    #[serde(alias = "style_css")]
    pub base_css: String,
    pub popup_css: String,
    pub panel_css: String,
    pub widgets_css: String,
    /// Border thickness for cards and controls (pixels).
    pub border_width: u8,
    /// Corner radius for notification cards (pixels).
    pub card_radius: u8,
    /// Base alpha for panel surfaces (0.0 - 1.0).
    pub surface_alpha: f32,
    /// Stronger alpha for panel surfaces (0.0 - 1.0).
    pub surface_strong_alpha: f32,
    /// Global alpha for card surfaces (0.0 - 1.0).
    pub card_alpha: f32,
    /// Alpha for softer drop shadows (0.0 - 1.0).
    pub shadow_soft_alpha: f32,
    /// Alpha for stronger drop shadows (0.0 - 1.0).
    pub shadow_strong_alpha: f32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            base_css: "base.css".to_string(),
            popup_css: "popup.css".to_string(),
            panel_css: "panel.css".to_string(),
            widgets_css: "widgets.css".to_string(),
            border_width: 1,
            card_radius: 16,
            surface_alpha: 0.88,
            surface_strong_alpha: 0.96,
            card_alpha: 0.94,
            shadow_soft_alpha: 0.30,
            shadow_strong_alpha: 0.55,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RuleConfig {
    /// Optional rule name for logging or debugging.
    pub name: Option<String>,
    /// Match against the notification app name (case-insensitive substring).
    pub app: Option<String>,
    /// Match against the notification summary (case-insensitive substring).
    pub summary: Option<String>,
    /// Match against the notification body (case-insensitive substring).
    pub body: Option<String>,
    /// Match against the notification category hint (case-insensitive substring).
    pub category: Option<String>,
    /// Match against urgency (0=low, 1=normal, 2=critical).
    pub urgency: Option<u8>,
    /// Suppress popups when true.
    pub no_popup: Option<bool>,
    /// Suppress sound when true.
    pub silent: Option<bool>,
    /// Force urgency when set (0=low, 1=normal, 2=critical).
    pub force_urgency: Option<u8>,
    /// Override expire timeout in milliseconds (-1 for default, 0 for no expire).
    pub expire_timeout_ms: Option<i64>,
    /// Override resident flag when set.
    pub resident: Option<bool>,
    /// Override transient flag when set.
    pub transient: Option<bool>,
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    Top,
    Bottom,
    Left,
    Right,
}

impl Default for Anchor {
    fn default() -> Self {
        // Default anchor determines which screen corner/edge the panel is attached to
        // when no explicit anchor is provided in config.
        Self::TopRight
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelKeyboardInteractivity {
    // Do not request keyboard focus; panel is purely pointer-driven.
    None,
    // Only request keyboard focus when an interaction requires it (search entry, text input, etc.).
    OnDemand,
    // Always grab exclusive keyboard focus while the panel is open.
    Exclusive,
}

impl Default for PanelKeyboardInteractivity {
    fn default() -> Self {
        // OnDemand is a good UX default: keyboard works when needed without stealing focus constantly.
        Self::OnDemand
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Margins {
    // Pixel margins applied around the panel/control-center surface.
    // These are logical pixels (before output scaling), and are used both for aesthetics and
    // to keep the surface off the screen edge / away from reserved work area.
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Default for Margins {
    fn default() -> Self {
        // Default padding around the panel. Keeping it symmetric produces a balanced look by default.
        // Users can override individual edges in config for tighter or asymmetric layouts.
        Self {
            top: 12,
            right: 12,
            bottom: 12,
            left: 12,
        }
    }
}
