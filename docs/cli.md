# CLI (noticenterctl)

`noticenterctl` talks to the UnixNotis control plane over D-Bus. The CLI is available as a
standalone binary or via `cargo run`.

## Common usage

```sh
# Open and close the panel
noticenterctl open-panel
noticenterctl close-panel

# Toggle DND
noticenterctl dnd toggle

# Clear all notifications
noticenterctl clear
```

If running from the workspace:

```sh
cargo run --release -p noticenterctl -- open-panel
```

## Commands

- `toggle-panel`
- `open-panel [--debug <level>]` where level is `critical|warn|info|verbose`
- `close-panel`
- `dnd <on|off|toggle>`
- `clear`
- `dismiss <id>`
- `list-active [--full]`
- `list-history [--full]`
- `inhibit <reason> [--scope <all|popups>]`
- `uninhibit <id>`
- `list-inhibitors`

### Debug logging

Use `open-panel --debug` to enable panel debug logging for the current session:

```sh
UNIXNOTIS_DIAGNOSTIC=1 noticenterctl open-panel --debug verbose
```

### Listing notifications

The `--full` flag emits full summaries and bodies when diagnostic mode is enabled:

```sh
UNIXNOTIS_DIAGNOSTIC=1 noticenterctl list-active --full
```

### Inhibit examples

```sh
# Suppress popups while a fullscreen app is running
TOKEN=$(noticenterctl inhibit "presentation" --scope popups)

# Later, re-enable popups
noticenterctl uninhibit "$TOKEN"

# Inspect current inhibitors
noticenterctl list-inhibitors
```
