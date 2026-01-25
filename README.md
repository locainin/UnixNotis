# UnixNotis

![Example output](assets/images/ExampleDisplay.png)

UnixNotis is a Wayland-first notification system with a control-center panel and toast popups.
It includes a D-Bus daemon that implements the Freedesktop.org notification spec and GTK4
frontends for the panel and popups.

## Features

- Freedesktop.org notification daemon with history, rules, sound, and DND.
- Persistent DND state across daemon restarts.
- Control-center panel with widgets, notification list, and media controls.
- Toast popup UI with configurable timeouts and styling.
- D-Bus inhibit API for programmatic popup suppression.
- MPRIS media integration with playback controls.
- Hot-reloaded config and CSS for fast iteration.
- CLI control via `noticenterctl`.

## Components

- `unixnotis-daemon`: D-Bus notification service, state store, history, and lifecycle control.
- `unixnotis-center`: Control-center panel UI with widgets, media controls, and notification list.
- `unixnotis-popups`: Toast popup UI for transient notifications.
- `noticenterctl`: CLI helper to open/close the panel and send control actions.
- `css-check`: Helper binary used by the center to validate CSS during reloads.

## Performance and optimization focus

- Event-driven watchers for toggles and sliders when possible.
- In-process readers for common stats to avoid shell spawns.
- Command budgeting with timeouts, concurrency limits, and jitter.
- Icon and media caching to avoid repeated decoding.
- Watchers paused when the panel is closed to avoid background load.

## Requirements

- Wayland session (panel UI requires Wayland compositors).
- GTK4 development libraries.
- gtk4-layer-shell library (pkg-config name: `gtk4-layer-shell-0`).
- `pkg-config` for native dependency discovery at build time.
- D-Bus session bus.
- Rust toolchain for builds and the installer.
- systemd --user for the installer-managed service.
- POSIX shell (`sh`) for widget commands that use pipes or redirects.
- Optional external commands used by widgets and watchers:
  - `wpctl` (WirePlumber) or `pactl` (pipewire-pulse / PulseAudio) for volume control and updates
  - `nmcli` for NetworkManager toggles
  - `brightnessctl` for the brightness slider
  - `bluetoothctl` for Bluetooth toggles
  - `dbus-monitor` for Bluetooth change events
  - `rfkill` for airplane mode toggles
  - `udevadm` for rfkill events
  - `hyprsunset` for night mode toggles on Hyprland
  - `hyprctl` for night mode IPC control on Hyprland
  - `pgrep`/`pkill` (procps) for night mode state/control
  - `pactl` for audio subscription events

### Distro packages

Arch Linux:

```sh
sudo pacman -S gtk4 gtk4-layer-shell pkgconf dbus systemd rust
```

Arch Linux (optional widget backends):

```sh
sudo pacman -S networkmanager wireplumber pipewire-pulse brightnessctl bluez rfkill procps-ng hyprsunset
```

## Getting started

Quick install or trial run:

```sh
git clone https://github.com/locainin/UnixNotis
cd UnixNotis
cargo run --release -p unixnotis-installer
```

Manual build/run:

```sh
cargo build --release
cargo run --release -p unixnotis-daemon
cargo run --release -p unixnotis-center
cargo run --release -p unixnotis-popups
```

Panel control:

```sh
cargo run --release -p noticenterctl -- open-panel
cargo run --release -p noticenterctl -- close-panel
```

## Documentation

- Getting started: `docs/getting-started.md`
- Configuration guide: `docs/configuration.md`
- Widgets and commands: `docs/widgets.md`
- CLI usage: `docs/cli.md`
- D-Bus API: `docs/dbus.md`

## Waybar integration

There is no built-in Waybar module. A custom module works well and is simple to configure.
Example snippet for `$HOME/.config/waybar/config`:

```json
{
  "custom/notifications": {
    "exec": "noticenterctl list-active | awk '{print $3}'",
    "interval": 2,
    "on-click": "noticenterctl toggle-panel",
    "tooltip": false
  }
}
```

Example CSS for `$HOME/.config/waybar/style.css`:

```css
#custom-notifications {
  padding: 0 10px;
}
```

## Systemd user unit (installer-managed)

The installer manages the user unit. The unit runs the daemon from `$HOME/.local/bin`:

```ini
[Unit]
Description=UnixNotis Daemon

[Service]
ExecStart=%h/.local/bin/unixnotis-daemon
Restart=on-failure

[Install]
WantedBy=default.target
```

The daemon launches the panel and popup frontends automatically.

## Logging

Log level is controlled by `general.log_level` in the config. Standard `RUST_LOG` overrides apply
when set in the environment.

UnixNotis redacts notification bodies and command output in logs by default. To opt in to
diagnostic logging, set `UNIXNOTIS_DIAGNOSTIC=1`. This enables capped, newline-stripped snippets
for debugging (limits are enforced to avoid leaking full content).

To stream logs in the terminal while opening the panel:

```sh
UNIXNOTIS_DIAGNOSTIC=1 noticenterctl open-panel --debug verbose
```

Valid levels are `critical`, `warn`, `info`, and `verbose`.

For CLI output that includes notification bodies, use `--full` with diagnostic mode enabled:

```sh
UNIXNOTIS_DIAGNOSTIC=1 noticenterctl list-active --full
```

## Development

```sh
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Troubleshooting

- Panel fails to start: ensure the session type is Wayland (`XDG_SESSION_TYPE=wayland`).
- Icons missing: verify GTK icon themes are installed and the image hints contain valid paths.
- Widget toggles do not update: ensure the optional external commands listed above are available.
