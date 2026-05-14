//! Shared command templates for widget defaults and runtime migrations

pub(crate) const WIFI_STATE_NMCLI: &str = "nmcli radio wifi";
pub(crate) const WIFI_ON_NMCLI: &str = "nmcli radio wifi on";
pub(crate) const WIFI_OFF_NMCLI: &str = "nmcli radio wifi off";
pub(crate) const WIFI_WATCH_NMCLI: &str = "nmcli -t monitor";

pub(crate) const BLUETOOTH_STATE_BLUETOOTHCTL: &str = "bluetoothctl show";
pub(crate) const BLUETOOTH_ON_BLUETOOTHCTL: &str = "bluetoothctl power on";
pub(crate) const BLUETOOTH_OFF_BLUETOOTHCTL: &str = "bluetoothctl power off";
// D-Bus monitor keeps updates flowing without a controlling TTY.
pub(crate) const BLUETOOTH_WATCH_DBUS: &str = "dbus-monitor --system type=signal,sender=org.bluez";

pub(crate) const AIRPLANE_STATE_CMD: &str =
    "rfkill list all | awk '/Soft blocked:/ { seen=1; if ($3 != \"yes\") bad=1 } END { exit (seen && !bad) ? 0 : 1 }'";
pub(crate) const AIRPLANE_ON_CMD: &str = "rfkill block all";
pub(crate) const AIRPLANE_OFF_CMD: &str = "rfkill unblock all";
pub(crate) const AIRPLANE_WATCH_CMD: &str = "udevadm monitor --udev --subsystem-match=rfkill";

pub(crate) const TOGGLE_KIND_WIFI: &str = "wifi";
pub(crate) const TOGGLE_KIND_BLUETOOTH: &str = "bluetooth";
pub(crate) const TOGGLE_KIND_AIRPLANE: &str = "airplane";
pub(crate) const TOGGLE_KIND_NIGHT: &str = "night";
