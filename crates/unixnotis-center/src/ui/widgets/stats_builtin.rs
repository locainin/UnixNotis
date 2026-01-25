//! In-process stats readers for common widgets.
//!
//! Reads system data from procfs/sysfs to avoid spawning shell commands.

use std::fs;
use std::path::Path;
use std::time::Instant;

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
                    let delta_total = total - *last_total;
                    let delta_idle = idle.saturating_sub(*last_idle);
                    100.0 * (delta_total.saturating_sub(delta_idle)) as f64 / delta_total as f64
                } else if total > 0 {
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

fn read_network(state: &mut BuiltinState, iface: &mut Option<String>) -> Option<String> {
    if iface.is_none() {
        *iface = pick_default_iface();
    }
    let iface = iface.as_ref()?;
    let (rx, tx) = read_network_bytes(iface)?;
    match state {
        BuiltinState::Network {
            last_rx,
            last_tx,
            last_at,
        } => {
            let now = Instant::now();
            let elapsed = now.duration_since(*last_at).as_secs_f64();
            let rx_rate = if elapsed > 0.0 {
                (rx.saturating_sub(*last_rx)) as f64 / elapsed
            } else {
                0.0
            };
            let tx_rate = if elapsed > 0.0 {
                (tx.saturating_sub(*last_tx)) as f64 / elapsed
            } else {
                0.0
            };
            *last_rx = rx;
            *last_tx = tx;
            *last_at = now;
            Some(format!(
                "RX {} TX {}",
                format_rate(rx_rate),
                format_rate(tx_rate)
            ))
        }
        _ => None,
    }
}

fn read_cpu_sample() -> Option<(u64, u64)> {
    let contents = fs::read_to_string("/proc/stat").ok()?;
    let line = contents.lines().find(|line| line.starts_with("cpu "))?;
    let mut parts = line.split_whitespace();
    let _cpu = parts.next()?;
    let mut values = Vec::new();
    for part in parts {
        if let Ok(value) = part.parse::<u64>() {
            values.push(value);
        }
    }
    if values.len() < 4 {
        return None;
    }
    let total: u64 = values.iter().sum();
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

fn read_memory() -> Option<String> {
    let contents = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total_kb = None;
    let mut avail_kb = None;
    for line in contents.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok());
        } else if line.starts_with("MemAvailable:") {
            avail_kb = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok());
        }
    }
    let total = total_kb? as f64 / 1024.0 / 1024.0;
    let avail = avail_kb? as f64 / 1024.0 / 1024.0;
    let used = (total - avail).max(0.0);
    Some(format!("{:.1}/{:.1} GB", used, total))
}

fn read_loadavg() -> Option<String> {
    let contents = fs::read_to_string("/proc/loadavg").ok()?;
    let mut parts = contents.split_whitespace();
    let one = parts.next()?;
    let five = parts.next()?;
    let fifteen = parts.next()?;
    Some(format!("{} {} {}", one, five, fifteen))
}

fn read_power_supply_value(path: &Path) -> Option<u64> {
    // Simple helper that trims and parses numeric power_supply values.
    let contents = fs::read_to_string(path).ok()?;
    contents.trim().parse::<u64>().ok()
}

fn read_battery() -> Option<String> {
    read_battery_from(Path::new("/sys/class/power_supply"))
}

fn read_battery_from(root: &Path) -> Option<String> {
    let entries = fs::read_dir(root).ok()?;
    let mut energy_now_total = 0u64;
    let mut energy_full_total = 0u64;
    let mut charge_now_total = 0u64;
    let mut charge_full_total = 0u64;
    let mut energy_count = 0u64;
    let mut charge_count = 0u64;
    let mut capacity_sum = 0u64;
    let mut capacity_count = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        let device_type = match fs::read_to_string(path.join("type")) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if device_type.trim() != "Battery" {
            continue;
        }
        // Skip batteries that report not present to avoid mixing placeholder devices.
        if let Some(present) = read_power_supply_value(&path.join("present")) {
            if present == 0 {
                continue;
            }
        }
        // Capture capacity regardless of unit family to support mixed-unit fallback.
        let capacity = read_power_supply_value(&path.join("capacity"));
        // Prefer energy_* or charge_* pairs so multi-battery systems aggregate correctly.
        // Capacity alone is a last-resort fallback because it is not weighted by size.
        if let (Some(now), Some(full)) = (
            read_power_supply_value(&path.join("energy_now")),
            read_power_supply_value(&path.join("energy_full")),
        ) {
            if full > 0 {
                energy_now_total = energy_now_total.saturating_add(now);
                energy_full_total = energy_full_total.saturating_add(full);
                energy_count = energy_count.saturating_add(1);
                if let Some(capacity) = capacity {
                    capacity_sum = capacity_sum.saturating_add(capacity);
                    capacity_count = capacity_count.saturating_add(1);
                }
                continue;
            }
        }
        if let (Some(now), Some(full)) = (
            read_power_supply_value(&path.join("charge_now")),
            read_power_supply_value(&path.join("charge_full")),
        ) {
            if full > 0 {
                charge_now_total = charge_now_total.saturating_add(now);
                charge_full_total = charge_full_total.saturating_add(full);
                charge_count = charge_count.saturating_add(1);
                if let Some(capacity) = capacity {
                    capacity_sum = capacity_sum.saturating_add(capacity);
                    capacity_count = capacity_count.saturating_add(1);
                }
                continue;
            }
        }
        if let Some(capacity) = capacity {
            capacity_sum = capacity_sum.saturating_add(capacity);
            capacity_count = capacity_count.saturating_add(1);
        }
    }
    // Do not mix energy and charge units; fall back to capacity if mixed.
    // If both unit families are present, capacity averaging is the only safe fallback.
    if energy_full_total > 0 && charge_count == 0 {
        // Rounded integer percent avoids floating-point drift for repeated reads.
        let percent =
            (energy_now_total.saturating_mul(100) + energy_full_total / 2) / energy_full_total;
        return Some(percent.to_string());
    }
    if charge_full_total > 0 && energy_count == 0 {
        // Charge-based values use the same arithmetic when energy data is absent.
        let percent =
            (charge_now_total.saturating_mul(100) + charge_full_total / 2) / charge_full_total;
        return Some(percent.to_string());
    }
    if capacity_count > 0 {
        // Average capacity is less accurate but avoids returning nothing on minimal systems.
        let avg = (capacity_sum + capacity_count / 2) / capacity_count;
        return Some(avg.to_string());
    }
    None
}

fn read_network_bytes(iface: &str) -> Option<(u64, u64)> {
    let base = Path::new("/sys/class/net").join(iface).join("statistics");
    let rx = fs::read_to_string(base.join("rx_bytes")).ok()?;
    let tx = fs::read_to_string(base.join("tx_bytes")).ok()?;
    let rx = rx.trim().parse::<u64>().ok()?;
    let tx = tx.trim().parse::<u64>().ok()?;
    Some((rx, tx))
}

fn pick_default_iface() -> Option<String> {
    // Collect interface metadata from sysfs before choosing a default.
    // Deterministic selection avoids flicker across restarts when multiple interfaces are present.
    let entries = fs::read_dir("/sys/class/net").ok()?;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let iface = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("lo");
        // Loopback should never be selected for bandwidth stats.
        if iface == "lo" {
            continue;
        }
        // Track operstate separately so sorting can prefer active interfaces.
        let operstate = fs::read_to_string(path.join("operstate")).unwrap_or_default();
        candidates.push(IfaceCandidate {
            name: iface.to_string(),
            operstate,
        });
    }

    pick_default_iface_from(&candidates)
}

#[derive(Debug, Clone)]
struct IfaceCandidate {
    // Interface name as reported by sysfs.
    name: String,
    // Raw operstate contents ("up", "down", etc), kept for ranking.
    operstate: String,
}

fn pick_default_iface_from(candidates: &[IfaceCandidate]) -> Option<String> {
    // Filter invalid entries early to keep ranking logic simple.
    let mut ranked: Vec<&IfaceCandidate> = candidates
        .iter()
        .filter(|candidate| !candidate.name.is_empty())
        .filter(|candidate| candidate.name != "lo")
        .collect();
    if ranked.is_empty() {
        return None;
    }

    // Sort deterministically: prefer active interfaces, then physical, then by name.
    ranked.sort_by(|left, right| iface_sort_key(left).cmp(&iface_sort_key(right)));

    ranked.first().map(|candidate| candidate.name.clone())
}

fn iface_sort_key(candidate: &IfaceCandidate) -> (u8, u8, &str) {
    // Active interfaces are sorted first; other states are considered less reliable.
    let up_rank = if candidate.operstate.trim() == "up" {
        0
    } else {
        1
    };
    // Physical interfaces are favored over virtual ones for default bandwidth stats.
    let class_rank = iface_class_rank(candidate.name.as_str());
    // Name order provides stable ties across runs and reboots.
    (up_rank, class_rank, candidate.name.as_str())
}

fn iface_class_rank(name: &str) -> u8 {
    // Common physical prefixes across distros and predictable interface naming.
    const PHYSICAL_PREFIXES: [&str; 6] = ["en", "eth", "wl", "wlan", "wlp", "wwan"];
    // Known virtual or container/VM prefixes that should be deprioritized.
    const VIRTUAL_PREFIXES: [&str; 11] = [
        "veth",
        "docker",
        "br",
        "virbr",
        "vmnet",
        "tap",
        "tun",
        "wg",
        "zt",
        "lo",
        "tailscale",
    ];

    if starts_with_any(name, &PHYSICAL_PREFIXES) {
        return 0;
    }
    if starts_with_any(name, &VIRTUAL_PREFIXES) {
        return 2;
    }
    // Unknown interfaces are treated as neutral and sort after physical ones.
    1
}

fn starts_with_any(name: &str, prefixes: &[&str]) -> bool {
    // Prefix matching keeps the logic lightweight and deterministic.
    prefixes.iter().any(|prefix| name.starts_with(prefix))
}

fn format_rate(rate: f64) -> String {
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut value = rate.max(0.0);
    let mut idx = 0;
    while value >= 1024.0 && idx < units.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{:.0} {}", value, units[idx])
    } else {
        format!("{:.1} {}", value, units[idx])
    }
}

fn extract_iface(cmd: &str) -> Option<String> {
    let marker = "/sys/class/net/";
    let start = cmd.find(marker)? + marker.len();
    let rest = &cmd[start..];
    let iface = rest.split('/').next()?.trim();
    if iface.is_empty() {
        None
    } else {
        Some(iface.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{pick_default_iface_from, read_battery_from, IfaceCandidate};
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
                ("capacity", "80"),
            ],
        );
        write_device(
            temp.path(),
            "BAT1",
            &[
                ("type", "Battery"),
                ("present", "1"),
                ("charge_now", "20"),
                ("charge_full", "40"),
                ("capacity", "60"),
            ],
        );
        let percent = read_battery_from(temp.path()).expect("battery percent missing");
        assert_eq!(percent, "70");
    }

    #[test]
    fn battery_skips_not_present_devices() {
        let temp = TempDir::new("unixnotis-battery-present");
        write_device(
            temp.path(),
            "BAT0",
            &[
                ("type", "Battery"),
                ("present", "0"),
                ("energy_now", "10"),
                ("energy_full", "20"),
                ("capacity", "10"),
            ],
        );
        write_device(
            temp.path(),
            "BAT1",
            &[
                ("type", "Battery"),
                ("present", "1"),
                ("energy_now", "20"),
                ("energy_full", "40"),
            ],
        );
        let percent = read_battery_from(temp.path()).expect("battery percent missing");
        assert_eq!(percent, "50");
    }

    #[test]
    fn default_iface_prefers_up_physical_over_virtual() {
        // Virtual interfaces should be deprioritized even when they are up.
        let candidates = vec![
            IfaceCandidate {
                name: "docker0".to_string(),
                operstate: "up".to_string(),
            },
            IfaceCandidate {
                name: "wlan0".to_string(),
                operstate: "up".to_string(),
            },
        ];

        let selected = pick_default_iface_from(&candidates).expect("iface selected");
        assert_eq!(selected, "wlan0");
    }

    #[test]
    fn default_iface_uses_deterministic_name_tiebreaker() {
        // Alphabetical ordering keeps selection stable when all candidates are up.
        let candidates = vec![
            IfaceCandidate {
                name: "wlp2s0".to_string(),
                operstate: "up".to_string(),
            },
            IfaceCandidate {
                name: "enp3s0".to_string(),
                operstate: "up".to_string(),
            },
        ];

        let selected = pick_default_iface_from(&candidates).expect("iface selected");
        assert_eq!(selected, "enp3s0");
    }

    #[test]
    fn default_iface_falls_back_to_physical_when_none_up() {
        // When no interfaces are up, prefer physical devices over virtual ones.
        let candidates = vec![
            IfaceCandidate {
                name: "veth1234".to_string(),
                operstate: "down".to_string(),
            },
            IfaceCandidate {
                name: "eth0".to_string(),
                operstate: "down".to_string(),
            },
        ];

        let selected = pick_default_iface_from(&candidates).expect("iface selected");
        assert_eq!(selected, "eth0");
    }
}
