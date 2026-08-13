//! Shared typed command templates for widget defaults and runtime migrations

use crate::CommandSpec;

pub fn wifi_state() -> CommandSpec {
    CommandSpec::direct("nmcli", ["radio", "wifi"])
}

pub fn wifi_on() -> CommandSpec {
    CommandSpec::direct("nmcli", ["radio", "wifi", "on"])
}

pub fn wifi_off() -> CommandSpec {
    CommandSpec::direct("nmcli", ["radio", "wifi", "off"])
}

pub fn wifi_watch() -> CommandSpec {
    CommandSpec::direct("nmcli", ["-t", "monitor"])
}

pub fn bluetooth_state() -> CommandSpec {
    CommandSpec::direct("bluetoothctl", ["show"])
}

pub fn bluetooth_on() -> CommandSpec {
    CommandSpec::direct("bluetoothctl", ["power", "on"])
}

pub fn bluetooth_off() -> CommandSpec {
    CommandSpec::direct("bluetoothctl", ["power", "off"])
}

pub fn bluetooth_watch() -> CommandSpec {
    CommandSpec::direct("dbus-monitor", ["--system", "type=signal,sender=org.bluez"])
}

pub fn airplane_state() -> CommandSpec {
    CommandSpec::direct("rfkill", ["--json"])
}

pub fn airplane_on() -> CommandSpec {
    CommandSpec::direct("rfkill", ["block", "all"])
}

pub fn airplane_off() -> CommandSpec {
    CommandSpec::direct("rfkill", ["unblock", "all"])
}

pub fn airplane_watch() -> CommandSpec {
    CommandSpec::direct("udevadm", ["monitor", "--udev", "--subsystem-match=rfkill"])
}

pub const TOGGLE_KIND_WIFI: &str = "wifi";
pub const TOGGLE_KIND_BLUETOOTH: &str = "bluetooth";
pub const TOGGLE_KIND_AIRPLANE: &str = "airplane";
pub const TOGGLE_KIND_NIGHT: &str = "night";
