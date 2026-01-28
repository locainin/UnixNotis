//! In-process stats readers for common widgets.
//!
//! Reads system data from procfs/sysfs to avoid spawning shell commands.

#[path = "stats_builtin_battery.rs"]
mod stats_builtin_battery;
#[path = "stats_builtin_cpu.rs"]
mod stats_builtin_cpu;
#[path = "stats_builtin_load.rs"]
mod stats_builtin_load;
#[path = "stats_builtin_memory.rs"]
mod stats_builtin_memory;
#[path = "stats_builtin_network.rs"]
mod stats_builtin_network;

use std::time::Instant;

use stats_builtin_battery::read_battery;
use stats_builtin_cpu::read_cpu_sample;
use stats_builtin_load::read_loadavg;
use stats_builtin_memory::read_memory;
use stats_builtin_network::{extract_iface, read_network};

#[derive(Clone, Debug)]
pub(super) struct BuiltinStat {
    kind: BuiltinStatKind,
    state: BuiltinState,
}

#[derive(Clone, Debug)]
enum BuiltinStatKind {
    Cpu,
    Memory,
    Load,
    Battery,
    Network { iface: Option<String> },
}

#[derive(Clone, Debug)]
enum BuiltinState {
    None,
    Cpu {
        last_total: u64,
        last_idle: u64,
    },
    Network {
        last_rx: u64,
        last_tx: u64,
        last_at: Instant,
    },
}

impl BuiltinStat {
    pub(super) fn from_command(cmd: &str) -> Option<Self> {
        let trimmed = cmd.trim();
        if let Some(rest) = trimmed.strip_prefix("builtin:") {
            // Explicit builtin tags bypass filesystem path sniffing.
            return Self::from_builtin_tag(rest);
        }
        if trimmed.contains("/proc/stat") {
            return Some(Self::new(BuiltinStatKind::Cpu));
        }
        if trimmed.contains("/proc/meminfo") {
            return Some(Self::new(BuiltinStatKind::Memory));
        }
        if trimmed.contains("/proc/loadavg") {
            return Some(Self::new(BuiltinStatKind::Load));
        }
        if trimmed.contains("/sys/class/power_supply") {
            return Some(Self::new(BuiltinStatKind::Battery));
        }
        if trimmed.contains("/sys/class/net") && trimmed.contains("statistics") {
            let iface = extract_iface(trimmed);
            return Some(Self::new(BuiltinStatKind::Network { iface }));
        }
        None
    }

    pub(super) fn read(&mut self) -> Option<String> {
        match &mut self.kind {
            BuiltinStatKind::Cpu => self.read_cpu(),
            BuiltinStatKind::Memory => read_memory(),
            BuiltinStatKind::Load => read_loadavg(),
            BuiltinStatKind::Battery => read_battery(),
            BuiltinStatKind::Network { iface } => read_network(&mut self.state, iface),
        }
    }

    fn new(kind: BuiltinStatKind) -> Self {
        let state = match kind {
            BuiltinStatKind::Cpu => BuiltinState::Cpu {
                last_total: 0,
                last_idle: 0,
            },
            BuiltinStatKind::Network { .. } => BuiltinState::Network {
                last_rx: 0,
                last_tx: 0,
                last_at: Instant::now(),
            },
            _ => BuiltinState::None,
        };
        Self { kind, state }
    }

    fn from_builtin_tag(tag: &str) -> Option<Self> {
        let mut parts = tag.split(':');
        let kind = parts.next()?.trim();
        match kind {
            "cpu" => Some(Self::new(BuiltinStatKind::Cpu)),
            "mem" | "memory" => Some(Self::new(BuiltinStatKind::Memory)),
            "load" => Some(Self::new(BuiltinStatKind::Load)),
            "battery" => Some(Self::new(BuiltinStatKind::Battery)),
            "net" => {
                let iface = parts.next().map(|value| value.to_string());
                Some(Self::new(BuiltinStatKind::Network { iface }))
            }
            _ => None,
        }
    }

    fn read_cpu(&mut self) -> Option<String> {
        let (total, idle) = read_cpu_sample()?;
        let usage = match &mut self.state {
            BuiltinState::Cpu {
                last_total,
                last_idle,
            } => {
                let usage = if *last_total > 0 && total > *last_total {
                    // Delta-based usage avoids spikes when the counter wraps.
                    let delta_total = total - *last_total;
                    let delta_idle = idle.saturating_sub(*last_idle);
                    100.0 * (delta_total.saturating_sub(delta_idle)) as f64 / delta_total as f64
                } else if total > 0 {
                    // First read falls back to absolute usage.
                    100.0 * (total.saturating_sub(idle)) as f64 / total as f64
                } else {
                    0.0
                };
                *last_total = total;
                *last_idle = idle;
                usage
            }
            _ => 0.0,
        };
        Some(format!("{:.0}%", usage.clamp(0.0, 100.0)))
    }
}

#[cfg(test)]
mod tests {
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
}
