# Widgets

Widgets are configured under `[widgets]` in `config.toml`. The panel renders sliders,
toggles, stats, and cards in that order. Widgets can be disabled by setting `enabled = false`
or by removing entries from the corresponding list.

## Refresh cadence

- `refresh_interval_ms`: fast polling interval (milliseconds).
- `refresh_interval_slow_ms`: slower interval used during stable periods.

Widgets with `watch_cmd` can update based on events and reduce polling. When a watch
command is missing or fails, the widget falls back to polling.

## Slider widgets

`[widgets.volume]` and `[widgets.brightness]` share the same schema:

```toml
[widgets.volume]
enabled = true
label = "Volume"
icon = "audio-volume-high-symbolic"
icon_muted = "audio-volume-muted-symbolic"
get_cmd = "wpctl get-volume @DEFAULT_AUDIO_SINK@"
set_cmd = "wpctl set-volume @DEFAULT_AUDIO_SINK@ {value}%"
toggle_cmd = "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"
# watch_cmd = "pactl subscribe"
min = 0.0
max = 100.0
step = 1.0
parse_mode = "auto" # auto | percent | ratio
```

Notes:
- `{value}` is replaced with the slider value.
- `parse_mode = "ratio"` treats `0.0..=1.0` output as a percentage.

## Toggle widgets

Each toggle entry describes commands for a binary control:

```toml
[[widgets.toggles]]
kind = "bluetooth"
label = "Bluetooth"
icon = "bluetooth-active-symbolic"
state_cmd = "bluetoothctl show"
on_cmd = "bluetoothctl power on"
off_cmd = "bluetoothctl power off"
watch_cmd = "dbus-monitor --system type=signal,sender=org.bluez"
```

Notes:
- `kind` is used to apply runtime defaults and can be omitted for custom toggles.
- `watch_cmd` is optional and may be disabled at runtime if unavailable.

## Stat widgets

Stats can use built-in readers or custom commands. Built-ins read from procfs/sysfs and
avoid process spawns.

Built-in tags:
- `builtin:cpu`
- `builtin:memory`
- `builtin:load`
- `builtin:battery`
- `builtin:net` or `builtin:net:INTERFACE`

Example:

```toml
[[widgets.stats]]
label = "CPU"
icon = "utilities-system-monitor-symbolic"
cmd = "builtin:cpu"
min_height = 72
```

If `cmd` is a shell command, its stdout is used as the widget value.

## Card widgets

Cards show multi-line content and are commonly used for calendar or custom scripts.

```toml
[[widgets.cards]]
kind = "calendar"
title = "Calendar"
subtitle = "Today"
icon = "x-office-calendar-symbolic"
cmd = "date '+%A, %B %d'"
min_height = 180
monospace = false
```

`cmd` output is rendered as the card body. When `cmd` is omitted and `kind` is a known
built-in (for example `calendar`), the card renders its internal data source.
