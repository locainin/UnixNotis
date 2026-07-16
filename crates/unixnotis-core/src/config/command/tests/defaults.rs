use super::super::defaults::{
    BLUETOOTH_WATCH_DBUS, TOGGLE_KIND_AIRPLANE, TOGGLE_KIND_BLUETOOTH, TOGGLE_KIND_NIGHT,
    TOGGLE_KIND_WIFI, WIFI_STATE_NMCLI,
};

#[test]
fn built_in_toggle_kinds_and_watch_commands_remain_nonempty() {
    for kind in [
        TOGGLE_KIND_WIFI,
        TOGGLE_KIND_BLUETOOTH,
        TOGGLE_KIND_AIRPLANE,
        TOGGLE_KIND_NIGHT,
    ] {
        assert!(!kind.is_empty());
    }
    assert!(WIFI_STATE_NMCLI.starts_with("nmcli "));
    assert!(BLUETOOTH_WATCH_DBUS.starts_with("dbus-monitor "));
}
