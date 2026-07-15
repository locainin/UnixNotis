# UnixNotis

![UnixNotis default control center](assets/images/DefaultConfig.png)

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
- Rust toolchain for source builds.
- `curl` for the installer's best-effort update check.

## Quick start

![Installer CLI](assets/images/InstallerCLI.png)

Download the current release tarball from
[GitHub Releases](https://github.com/locainin/UnixNotis/releases), extract it, then run:

```sh
./unixnotis-installer
```

Release archives include the installer and the bundled runtime binaries, so they do not require a
Rust toolchain on the target system.

Source checkouts can still run the installer directly:

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

When launched from a downloaded release, the installer verifies the bundled binaries and then copies
them into `$HOME/.local/bin` instead of building from source. The TUI shows the installed version and
reports when a newer GitHub release is available.

Maintainers can build a local release archive manually:

```sh
scripts/package-release.sh v1.1.4
```

## Development

```sh
cargo test --workspace
cargo clippy --all-targets --all-features -- \
  -D warnings \
  -W clippy::pedantic \
  -W clippy::nursery \
  -W clippy::undocumented_unsafe_blocks \
  -W clippy::multiple_unsafe_ops_per_block \
  -W clippy::mem_forget \
  -W clippy::cast_ptr_alignment \
  -W clippy::transmute_ptr_to_ptr \
  -W clippy::fn_to_numeric_cast_any \
  -W clippy::as_pointer_underscore \
  -W clippy::lossy_float_literal
```

## Contributing

Want to help improve UnixNotis?

Please View Our [CONTRIBUTING.md](CONTRIBUTING.md) For Contribution Guidelines and Expectations.

## License

MIT. See `LICENSE`.
