//! Shared command templates for widget defaults and runtime migrations

pub const WIFI_STATE_NMCLI: &str = "nmcli radio wifi";
pub const WIFI_ON_NMCLI: &str = "nmcli radio wifi on";
pub const WIFI_OFF_NMCLI: &str = "nmcli radio wifi off";
pub const WIFI_WATCH_NMCLI: &str = "nmcli -t monitor";

pub const BLUETOOTH_STATE_BLUETOOTHCTL: &str = "bluetoothctl show";
pub const BLUETOOTH_ON_BLUETOOTHCTL: &str = "bluetoothctl power on";
pub const BLUETOOTH_OFF_BLUETOOTHCTL: &str = "bluetoothctl power off";
// D-Bus monitoring keeps updates flowing without a controlling terminal
pub const BLUETOOTH_WATCH_DBUS: &str = "dbus-monitor --system type=signal,sender=org.bluez";

pub const AIRPLANE_STATE_CMD: &str =
    "rfkill list all | awk '/Soft blocked:/ { seen=1; if ($3 != \"yes\") bad=1 } END { exit (seen && !bad) ? 0 : 1 }'";
pub const AIRPLANE_ON_CMD: &str = "rfkill block all";
pub const AIRPLANE_OFF_CMD: &str = "rfkill unblock all";
pub const AIRPLANE_WATCH_CMD: &str = "udevadm monitor --udev --subsystem-match=rfkill";

pub const TOGGLE_KIND_WIFI: &str = "wifi";
pub const TOGGLE_KIND_BLUETOOTH: &str = "bluetooth";
pub const TOGGLE_KIND_AIRPLANE: &str = "airplane";
pub const TOGGLE_KIND_NIGHT: &str = "night";
