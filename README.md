# UnixNotis

![Example output](assets/images/ExampleDisplay.png)

UnixNotis is a Wayland-first notification system with a D-Bus daemon, a control-center
panel, toast popups, shareable presets, and installer-managed startup.

## Documentation

Documentation lives in the GitHub Wiki (see the **Wiki** tab on the repository).

Clone the wiki locally if needed:

```sh
git clone https://github.com/locainin/UnixNotis.wiki.git
```

### Configuration and Styling

[See the wiki for more details.](https://github.com/locainin/UnixNotis/wiki)

## Features

- Freedesktop.org notification daemon with history, rules, sound, and DND.
- Persistent DND state across daemon restarts.
- Control-center panel with widgets, notification list, and media controls.
- Toast popup UI with configurable timeouts and styling.
- D-Bus inhibit API for programmatic popup suppression.
- MPRIS media integration with playback controls.
- Hot-reloaded config and CSS for fast iteration.
- CLI control via `noticenterctl`.
- Installer-managed startup through `systemd --user`, `dinit --user`, user runit, or user s6-rc.
- Preset export/import tooling for sharing config, theme, scripts, and assets.

## Requirements

- Wayland session.
- GTK4 + gtk4-layer-shell (pkg-config: `gtk4-layer-shell-0`).
- D-Bus session bus.
- A supported user service manager for installer-managed startup.
  - Default: `systemd --user`.
  - Alternatives: `dinit --user`, user runit, or user s6-rc.
- Rust toolchain for builds and the installer.

## Quick start

![Installer CLI](assets/images/InstallerCLI.png)

```sh
git clone https://github.com/locainin/UnixNotis
cd UnixNotis
cargo run --release -p unixnotis-installer
```

Non-systemd setups can choose a backend explicitly:

```sh
cargo run --release -p unixnotis-installer -- --service-manager dinit
cargo run --release -p unixnotis-installer -- --service-manager runit
cargo run --release -p unixnotis-installer -- --service-manager s6
```

The installer builds the release binaries, installs them under `$HOME/.local/bin`, writes the
default config/theme files, syncs the live Wayland session environment, and enables the selected
user service.

## Development

```sh
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W clippy::restriction
```

## Contributing

Want to help improve UnixNotis?

Please View Our [CONTRIBUTING.md](CONTRIBUTING.md) For Contribution Guidelines and Expectations.

## License

MIT. See `LICENSE`.
