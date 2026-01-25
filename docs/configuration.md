# Configuration

UnixNotis loads `config.toml` from the XDG config directory:

- `$XDG_CONFIG_HOME/unixnotis/config.toml`
- fallback: `$HOME/.config/unixnotis/config.toml`

If the file is missing, built-in defaults are used. Unknown keys are ignored with a warning.
Runtime sanitization clamps out-of-range values to safe limits.

Theme CSS files are stored alongside the config directory and are created on demand
when missing. Editing these files is the primary way to adjust styling.

## Minimal example

```toml
[general]
# Optional: use tracing-subscriber filter syntax.
log_level = "info"
# Default DND state when no persisted value exists.
dnd_default = false

[inhibit]
# "no_popups" (default) or "drop_all"
mode = "no_popups"

[panel]
anchor = "right"
width = 420
height = 0
keyboard_interactivity = "on-demand"
output = "HDMI-A-1"
close_on_blur = false
close_on_click_outside = true
respect_work_area = true

[panel.margin]
top = 54
right = 6
bottom = 6
left = 6

[popups]
anchor = "top-right"
width = 360
spacing = 12
max_visible = 4
default_timeout_ms = 5000
critical_timeout_ms = 10000
allow_click_through = false
output = "HDMI-A-1"

[popups.margin]
top = 12
right = 12
bottom = 12
left = 12

[history]
max_entries = 200
max_active = 500
transient_to_history = false

[media]
enabled = true
include_browsers = true
browser_tokens = ["firefox", "chromium", "chrome"]
title_char_limit = 32
allowlist = []
denylist = ["playerctld"]

[sound]
enabled = true
default_name = "message-new-instant"
# default_file and default_dir resolve relative to the config dir if set
# default_file = "sounds/notify.ogg"
# default_dir = "sounds"

[theme]
base_css = "base.css"
popup_css = "popup.css"
panel_css = "panel.css"
widgets_css = "widgets.css"
border_width = 1
card_radius = 16
surface_alpha = 0.88
surface_strong_alpha = 0.96
card_alpha = 0.94
shadow_soft_alpha = 0.30
shadow_strong_alpha = 0.55

[widgets]
refresh_interval_ms = 1000
refresh_interval_slow_ms = 3000

[widgets.volume]
enabled = true
label = "Volume"
icon = "audio-volume-high-symbolic"
icon_muted = "audio-volume-muted-symbolic"
get_cmd = "wpctl get-volume @DEFAULT_AUDIO_SINK@"
set_cmd = "wpctl set-volume @DEFAULT_AUDIO_SINK@ {value}%"
# toggle_cmd and watch_cmd are optional

[widgets.brightness]
enabled = true
label = "Brightness"
icon = "display-brightness-symbolic"
get_cmd = "brightnessctl -m"
set_cmd = "brightnessctl s {value}%"
# watch_cmd = "brightnessctl -w"

[[widgets.toggles]]
kind = "wifi"
label = "Wi-Fi"
icon = "network-wireless-signal-excellent-symbolic"
state_cmd = "nmcli radio wifi"
on_cmd = "nmcli radio wifi on"
off_cmd = "nmcli radio wifi off"
watch_cmd = "nmcli -t monitor"

[[widgets.stats]]
label = "CPU"
icon = "utilities-system-monitor-symbolic"
cmd = "builtin:cpu"
min_height = 72

[[widgets.cards]]
kind = "calendar"
title = "Calendar"
icon = "x-office-calendar-symbolic"
min_height = 180

[[rules]]
name = "silent-slack"
app = "slack"
silent = true
no_popup = true
```

## Section reference

### [general]
- `log_level`: optional tracing filter (same syntax as `RUST_LOG`).
- `dnd_default`: default DND state when no persisted state exists.

### [inhibit]
- `mode`: `"no_popups"` (store notifications but suppress popups) or `"drop_all"` (ignore notifications while inhibited).

### [panel]
- `anchor`: one of `top-right`, `top-left`, `bottom-right`, `bottom-left`, `top`, `bottom`, `left`, `right`.
- `width`: width in logical pixels.
- `height`: 0 uses compositor-driven height.
- `keyboard_interactivity`: `none`, `on-demand`, or `exclusive`.
- `output`: optional monitor name (Wayland output).
- `close_on_blur`: hide when panel loses focus.
- `close_on_click_outside`: hide on outside click (Hyprland only).
- `respect_work_area`: respects compositor reserved area (Hyprland only).
- `empty_text`: empty-state label text (supports `\n` for multi-line).
- `empty_offset_top`: top offset for empty-state label (logical pixels). Ignored when no widgets are visible; the label is centered.
- `[panel.margin]`: margins in logical pixels.

### [popups]
- `anchor`, `width`, `spacing`, `max_visible`.
- `default_timeout_ms`, `critical_timeout_ms` (optional).
- `allow_click_through`: disable input handling when true.
- `output`: optional monitor name.
- `[popups.margin]`: margins in logical pixels.

### [history]
- `max_entries`: history capacity.
- `max_active`: active list capacity.
- `transient_to_history`: includes transient notifications in history.

### [media]
- `enabled`, `include_browsers`.
- `browser_tokens`: case-insensitive tokens used to classify browser players.
- `title_char_limit`: marquee starts after this length.
- `allowlist` / `denylist`: case-insensitive substrings for player identities.

### [sound]
- `enabled`.
- `default_name`: freedesktop sound theme name.
- `default_file`: sound file path (relative to config dir when not absolute).
- `default_dir`: directory containing sound files.

### [theme]
Theme CSS file names resolve relative to the config directory. Alpha values are clamped to
`0.0..=1.0` and dimensions are clamped to safe ranges.

### [widgets]
See `docs/widgets.md` for the widget schema and command examples.

### [[rules]]
Rules match notification fields and apply overrides.

Fields:
- `name`: optional rule label.
- `app`, `summary`, `body`, `category`: case-insensitive substring matches.
- `urgency`: `low`, `normal`, or `critical` (or `0`, `1`, `2`).
- `no_popup`: suppress popups.
- `silent`: suppress sound.
- `force_urgency`: override urgency.
- `expire_timeout_ms`: override timeout (`-1` default, `0` never expire).
- `resident`: force resident flag.
- `transient`: force transient flag.
