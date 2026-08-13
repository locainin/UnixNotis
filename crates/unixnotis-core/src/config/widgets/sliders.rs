use serde::{Deserialize, Serialize};

use crate::CommandSpec;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SliderWidgetConfig {
    // Field-level defaults preserve the original one-line slider when old configs omit decoration
    // Newly generated sliders still receive the richer values from their constructors below
    pub enabled: bool,
    pub label: String,
    pub icon: String,
    pub icon_muted: Option<String>,
    pub get_cmd: CommandSpec,
    pub set_cmd: CommandSpec,
    pub toggle_cmd: Option<CommandSpec>,
    pub watch_cmd: Option<CommandSpec>,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Show the current numeric value at the end of the row
    pub show_value: bool,
    /// Optional decorative segment count below the slider track
    pub segments: usize,
    /// Show min/max sublabels below the slider track
    pub show_sublabels: bool,
    /// Left sublabel. Empty uses the slider min value
    pub sublabel_min: String,
    /// Right sublabel. Empty uses the slider max value
    pub sublabel_max: String,
    /// Controls how numeric command output is interpreted for slider values
    pub parse_mode: NumericParseMode,
}

impl SliderWidgetConfig {
    // wpctl is the stock PipeWire path and stays shell-free for the common case
    pub(in crate::config) fn wpctl_get() -> CommandSpec {
        CommandSpec::direct("wpctl", ["get-volume", "@DEFAULT_AUDIO_SINK@"])
    }

    pub(in crate::config) fn wpctl_set() -> CommandSpec {
        CommandSpec::direct("wpctl", ["set-volume", "@DEFAULT_AUDIO_SINK@", "{value}%"])
    }

    pub(in crate::config) fn wpctl_toggle() -> CommandSpec {
        CommandSpec::direct("wpctl", ["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
    }

    // pactl supports both PulseAudio and pipewire-pulse setups
    pub(in crate::config) fn pactl_get() -> CommandSpec {
        CommandSpec::shell(
            "pactl get-sink-volume @DEFAULT_SINK@; pactl get-sink-mute @DEFAULT_SINK@",
        )
    }

    pub(in crate::config) fn pactl_set() -> CommandSpec {
        CommandSpec::direct("pactl", ["set-sink-volume", "@DEFAULT_SINK@", "{value}%"])
    }

    pub(in crate::config) fn pactl_toggle() -> CommandSpec {
        CommandSpec::direct("pactl", ["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
    }

    // Long-running watcher used only when runtime detection confirms pactl exists
    pub(in crate::config) fn pactl_watch() -> CommandSpec {
        CommandSpec::direct("pactl", ["subscribe"])
    }

    pub(super) fn default_volume() -> Self {
        Self {
            enabled: true,
            label: "Volume".to_string(),
            icon: "audio-volume-high-symbolic".to_string(),
            icon_muted: Some("audio-volume-muted-symbolic".to_string()),
            // Runtime migration may switch these to pactl only for untouched stock config
            get_cmd: Self::wpctl_get(),
            set_cmd: Self::wpctl_set(),
            toggle_cmd: Some(Self::wpctl_toggle()),
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
            get_cmd: CommandSpec::direct("brightnessctl", ["-m"]),
            set_cmd: CommandSpec::direct("brightnessctl", ["s", "{value}%"]),
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

#[cfg(test)]
#[path = "tests/sliders.rs"]
mod tests;
