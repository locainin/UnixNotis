use super::should_emit_watch_event;

#[test]
fn pactl_watch_filters_to_sink_and_server() {
    let cmd = "pactl subscribe";
    assert!(should_emit_watch_event(cmd, "Event 'change' on sink #1"));
    assert!(should_emit_watch_event(cmd, "Event 'new' on server"));
    assert!(!should_emit_watch_event(cmd, "Event 'change' on source #2"));
    assert!(!should_emit_watch_event(
        cmd,
        "Event 'change' on sink-input #3"
    ));
}

#[test]
fn monitor_startup_noise_does_not_emit_watch_events() {
    assert!(!should_emit_watch_event(
        "nmcli -t monitor",
        "NetworkManager is running"
    ));
    assert!(!should_emit_watch_event(
        "udevadm monitor --udev --subsystem-match=rfkill",
        "monitor will print the received events for:"
    ));
    assert!(!should_emit_watch_event(
        "dbus-monitor --system type=signal,sender=org.bluez",
        "signal time=1 sender=org.freedesktop.DBus -> destination=:1.1 serial=1 path=/org/freedesktop/DBus; interface=org.freedesktop.DBus; member=NameAcquired"
    ));
}

#[test]
fn monitor_real_events_still_emit() {
    assert!(should_emit_watch_event(
        "nmcli -t monitor",
        "wlp0s20f3: connected"
    ));
    assert!(should_emit_watch_event(
        "udevadm monitor --udev --subsystem-match=rfkill",
        "UDEV  [1.0] change   /devices/platform/rfkill/rfkill0 (rfkill)"
    ));
    assert!(should_emit_watch_event(
        "dbus-monitor --system type=signal,sender=org.bluez",
        "signal time=1 sender=org.bluez path=/org/bluez; interface=org.freedesktop.DBus.Properties; member=PropertiesChanged"
    ));
    assert!(should_emit_watch_event(
        "dbus-monitor --system type=signal,sender=org.bluez",
        "signal time=1 sender=org.bluez path=/org/bluez; interface=org.bluez.Adapter1; member=NameLost"
    ));
}

#[test]
fn custom_watch_commands_emit_non_empty_lines() {
    assert!(should_emit_watch_event("echo test", "anything"));
    assert!(!should_emit_watch_event("echo test", ""));
}
