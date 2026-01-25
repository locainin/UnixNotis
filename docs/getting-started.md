# Getting Started

This guide covers build, install, and first-run workflows for UnixNotis.

## Quick build and run

```sh
cargo build --release
```

Run the daemon and UIs manually (requires a Wayland session):

```sh
cargo run --release -p unixnotis-daemon
cargo run --release -p unixnotis-center
cargo run --release -p unixnotis-popups
```

Open or close the panel:

```sh
cargo run --release -p noticenterctl -- open-panel
cargo run --release -p noticenterctl -- close-panel
```

## Installer (recommended)

The installer builds, installs, and manages the systemd user service:

```sh
cargo run --release -p unixnotis-installer
```

Installer actions:
- builds release binaries
- installs to `$HOME/.local/bin`
- writes config and theme files under `$XDG_CONFIG_HOME/unixnotis`
- installs and enables the systemd user unit

## Preview without install

Use trial mode to test without replacing the system service permanently:

```sh
cargo run --release -p unixnotis-installer
```

Select “Trial run” to temporarily replace the current notification daemon, then
restore it on exit.

## Configuration and widgets

- Configuration guide: `docs/configuration.md`
- Widgets guide: `docs/widgets.md`
- CLI usage: `docs/cli.md`
- D-Bus API: `docs/dbus.md`

## First-run checklist

- Ensure the Wayland session is active.
- Confirm GTK4 + gtk4-layer-shell are installed.
- Confirm the panel and popups are allowed by the compositor.
- Run `noticenterctl open-panel` to validate the control plane.

