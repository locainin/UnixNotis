# UnixNotis D-Bus Interfaces

This document describes the stable D-Bus surface for UnixNotis. The daemon exposes two
interfaces on the user session bus: the Freedesktop.org notification interface and the
UnixNotis control plane.

## Stability policy

- **Additive changes** (new methods, fields, signals) are considered non-breaking.
- **Removing or changing** existing methods, fields, or semantics is a breaking change.

## org.freedesktop.Notifications

- **Bus name:** `org.freedesktop.Notifications`
- **Object path:** `/org/freedesktop/Notifications`
- **Interface:** `org.freedesktop.Notifications`

This interface follows the Freedesktop.org notification specification, including
`Notify`, `CloseNotification`, and notification capability reporting.

### Introspection

```sh
busctl --user introspect org.freedesktop.Notifications /org/freedesktop/Notifications
```

## com.unixnotis.Control

- **Bus name:** `com.unixnotis.Control`
- **Object path:** `/com/unixnotis/Control`
- **Interface:** `com.unixnotis.Control`

### State

`ControlState` contains the daemon control-plane state:

- `dnd_enabled` (bool): current Do Not Disturb state.
- `history_count` (u32): total history entries.
- `inhibited` (bool): true when popups are suppressed by an inhibitor.
- `inhibitor_count` (u32): total active inhibitors.

### Methods

- `GetState() -> (ControlState)`
- `ListActive() -> (NotificationView[])`
- `ListHistory() -> (NotificationView[])`
- `OpenPanel() -> ()`
- `OpenPanelDebug(level: PanelDebugLevel) -> ()`
- `ClosePanel() -> ()`
- `TogglePanel() -> ()`
- `SetDnd(enabled: bool) -> ()`
- `Dismiss(id: u32) -> ()`
- `InvokeAction(id: u32, action_key: string) -> ()`
- `ClearAll() -> ()`
- `Inhibit(reason: string, scope: u32) -> (u64)`
- `Uninhibit(id: u64) -> ()`
- `ListInhibitors() -> ([(u64, string, u32, string)])`

### Signals

- `NotificationAdded(notification: NotificationView, show_popup: bool)`
- `NotificationUpdated(notification: NotificationView, show_popup: bool)`
- `NotificationClosed(id: u32, reason: CloseReason)`
- `StateChanged(state: ControlState)`
- `InhibitorsChanged(active: bool, count: u32)`
- `PanelRequested(request: PanelRequest)`

### Inhibit semantics

- `Inhibit` returns a unique token for the caller.
- Inhibitors are **owned by the unique bus name** (for example `:1.42`).
- `Uninhibit` only succeeds for the owning connection. Owner mismatch returns an
  access denied error.
- Unknown inhibitor IDs are treated as a no-op.
- When the owning client disconnects, its inhibitors are removed automatically.

#### Scope values

`scope` is a u32 with the following meaning:

- `0`: inhibit all notification output (legacy/default).
- `1`: inhibit popup rendering.

Additional values are reserved for future expansion.

### Popup suppression mode

The configuration section `inhibit.mode` controls how inhibited notifications are
handled:

- `no_popups` (default): notifications are stored and emitted, but popups are
  suppressed while inhibited.
- `drop_all`: notifications are ignored while inhibited (no store, no popups).

### DND persistence

The daemon persists DND state to:

- `$XDG_STATE_HOME/unixnotis/state.json`, or
- `$HOME/.local/state/unixnotis/state.json` if `XDG_STATE_HOME` is unset.

Writes are atomic (temp + rename) and best-effort durable across power loss.

### Introspection

```sh
busctl --user introspect com.unixnotis.Control /com/unixnotis/Control
```

### Client guidance

- Subscribe to control signals before issuing `GetState`/`ListActive`/`ListHistory` so
  the match rules are installed early and events are buffered during the seed round.
