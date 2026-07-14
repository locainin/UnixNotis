use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SliderWidgetConfig {
    // Field-level defaults preserve the original one-line slider when old configs omit decoration
    // Newly generated sliders still receive the richer values from their constructors below
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
    /// Show the current numeric value at the end of the row
    pub show_value: bool,
    /// Optional decorative segment count below the slider track
    #[serde(default = "legacy_slider_segments")]
    pub segments: usize,
    /// Show min/max sublabels below the slider track
    #[serde(default = "legacy_slider_sublabel_visibility")]
    pub show_sublabels: bool,
    /// Left sublabel. Empty uses the slider min value
    #[serde(default = "legacy_slider_sublabel")]
    pub sublabel_min: String,
    /// Right sublabel. Empty uses the slider max value
    #[serde(default = "legacy_slider_sublabel")]
    pub sublabel_max: String,
    /// Controls how numeric command output is interpreted for slider values
    pub parse_mode: NumericParseMode,
}

// These helpers apply only during deserialization of missing keys
// Keeping them separate from the constructors lets the stock theme evolve without rewriting presets

// Legacy slider rows used one continuous track without decorative segments
const fn legacy_slider_segments() -> usize {
    0
}

// Legacy slider rows did not reserve a second line for min and max labels
const fn legacy_slider_sublabel_visibility() -> bool {
    false
}

// Empty text lets the legacy hidden-sublabel behavior remain data-free
const fn legacy_slider_sublabel() -> String {
    String::new()
}

impl SliderWidgetConfig {
    // wpctl is the stock PipeWire path and stays shell-free for the common case
    pub(in crate::config) const WPCTL_GET: &'static str = "wpctl get-volume @DEFAULT_AUDIO_SINK@";
    pub(in crate::config) const WPCTL_SET: &'static str =
        "wpctl set-volume @DEFAULT_AUDIO_SINK@ {value}%";
    pub(in crate::config) const WPCTL_TOGGLE: &'static str =
        "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle";

    // pactl supports both PulseAudio and pipewire-pulse setups
    pub(in crate::config) const PACTL_GET: &'static str =
        "pactl get-sink-volume @DEFAULT_SINK@; pactl get-sink-mute @DEFAULT_SINK@";
    pub(in crate::config) const PACTL_SET: &'static str =
        "pactl set-sink-volume @DEFAULT_SINK@ {value}%";
    pub(in crate::config) const PACTL_TOGGLE: &'static str =
        "pactl set-sink-mute @DEFAULT_SINK@ toggle";

    // Long-running watcher used only when runtime detection confirms pactl exists
    pub(in crate::config) const PACTL_WATCH: &'static str = "pactl subscribe";

    pub(super) fn default_volume() -> Self {
        Self {
            enabled: true,
            label: "Volume".to_string(),
            icon: "audio-volume-high-symbolic".to_string(),
            icon_muted: Some("audio-volume-muted-symbolic".to_string()),
            // Runtime migration may switch these to pactl only for untouched stock config
            get_cmd: Self::WPCTL_GET.to_string(),
            set_cmd: Self::WPCTL_SET.to_string(),
            toggle_cmd: Some(Self::WPCTL_TOGGLE.to_string()),
            // None avoids writing a watcher that may not exist on the target host
            watch_cmd: None,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            show_value: true,
            // Ten segments give quick visual feedback without changing slider behavior
            segments: 10,
            show_sublabels: true,
            sublabel_min: "MUTE".to_string(),
            sublabel_max: "MAX".to_string(),
            parse_mode: NumericParseMode::Auto,
        }
    }

    pub(super) fn default_brightness() -> Self {
        Self {
            enabled: true,
            label: "Brightness".to_string(),
            icon: "display-brightness-symbolic".to_string(),
            icon_muted: None,
            // -m keeps brightnessctl output stable enough for the shared parser
            get_cmd: "brightnessctl -m".to_string(),
            set_cmd: "brightnessctl s {value}%".to_string(),
            toggle_cmd: None,
            // brightnessctl has no reliable stock watch mode, so polling remains explicit
            watch_cmd: None,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            show_value: true,
            // Brightness mirrors the volume scale for a consistent control rhythm
            segments: 10,
            show_sublabels: true,
            sublabel_min: "MIN".to_string(),
            sublabel_max: "MAX".to_string(),
            parse_mode: NumericParseMode::Auto,
        }
    }
}

impl Default for SliderWidgetConfig {
    fn default() -> Self {
        Self::default_volume()
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NumericParseMode {
    /// Uses heuristic parsing for mixed output formats
    #[default]
    Auto,
    /// Interprets values as percentages without normalization
    Percent,
    /// Interprets values as 0.0-1.0 ratios and scales to percent
    Ratio,
}
