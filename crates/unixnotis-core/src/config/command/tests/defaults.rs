use super::super::defaults::{
    bluetooth_watch, wifi_state, TOGGLE_KIND_AIRPLANE, TOGGLE_KIND_BLUETOOTH, TOGGLE_KIND_NIGHT,
    TOGGLE_KIND_WIFI,
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
    assert_eq!(
        wifi_state().program().and_then(|path| path.to_str()),
        Some("nmcli")
    );
    assert_eq!(
        bluetooth_watch().program().and_then(|path| path.to_str()),
        Some("dbus-monitor")
    );
}
