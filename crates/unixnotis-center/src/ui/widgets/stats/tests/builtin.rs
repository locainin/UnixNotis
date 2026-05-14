use super::stats_builtin_battery::read_battery_from;
use super::stats_builtin_network::{pick_default_iface_from, IfaceCandidate};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), stamp));
        fs::create_dir_all(&path).expect("temp dir creation failed");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best-effort cleanup to avoid leaving test artifacts on disk.
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_device(root: &Path, name: &str, entries: &[(&str, &str)]) {
    let device_path = root.join(name);
    fs::create_dir_all(&device_path).expect("device directory creation failed");
    for (file, contents) in entries {
        fs::write(device_path.join(file), contents).expect("device file write failed");
    }
}

#[test]
fn battery_energy_aggregates_weighted() {
    let temp = TempDir::new("unixnotis-battery-energy");
    write_device(
        temp.path(),
        "BAT0",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("energy_now", "30"),
            ("energy_full", "60"),
        ],
    );
    write_device(
        temp.path(),
        "BAT1",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("energy_now", "10"),
            ("energy_full", "40"),
        ],
    );
    let percent = read_battery_from(temp.path()).expect("battery percent missing");
    assert_eq!(percent, "40");
}

#[test]
fn battery_mixed_units_falls_back_to_capacity() {
    let temp = TempDir::new("unixnotis-battery-mixed");
    write_device(
        temp.path(),
        "BAT0",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("energy_now", "30"),
            ("energy_full", "60"),
            ("capacity", "60"),
        ],
    );
    write_device(
        temp.path(),
        "BAT1",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("charge_now", "10"),
            ("charge_full", "40"),
            ("capacity", "25"),
        ],
    );
    let percent = read_battery_from(temp.path()).expect("battery percent missing");
    assert_eq!(percent, "43");
}

#[test]
fn battery_skips_not_present_devices() {
    let temp = TempDir::new("unixnotis-battery-absent");
    write_device(
        temp.path(),
        "BAT0",
        &[
            ("type", "Battery"),
            ("present", "0"),
            ("energy_now", "30"),
            ("energy_full", "60"),
        ],
    );
    assert!(read_battery_from(temp.path()).is_none());
}

#[test]
fn default_iface_prefers_up_physical_over_virtual() {
    let candidates = vec![
        IfaceCandidate {
            name: "veth0".to_string(),
            operstate: "up".to_string(),
        },
        IfaceCandidate {
            name: "wlan0".to_string(),
            operstate: "up".to_string(),
        },
    ];
    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("wlan0".to_string())
    );
}

#[test]
fn default_iface_falls_back_to_physical_when_none_up() {
    let candidates = vec![
        IfaceCandidate {
            name: "eth0".to_string(),
            operstate: "down".to_string(),
        },
        IfaceCandidate {
            name: "docker0".to_string(),
            operstate: "up".to_string(),
        },
    ];
    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("eth0".to_string())
    );
}

#[test]
fn default_iface_uses_deterministic_name_tiebreaker() {
    let candidates = vec![
        IfaceCandidate {
            name: "eth1".to_string(),
            operstate: "down".to_string(),
        },
        IfaceCandidate {
            name: "eth0".to_string(),
            operstate: "down".to_string(),
        },
    ];
    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("eth0".to_string())
    );
}
