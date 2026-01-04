//! In-process stats readers for common widgets.
//!
//! Reads system data from procfs/sysfs to avoid spawning shell commands.

use std::fs;
use std::path::Path;
use std::time::Instant;

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
    Cpu { last_total: u64, last_idle: u64 },
    Network { last_rx: u64, last_tx: u64, last_at: Instant },
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
            total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok());
        } else if line.starts_with("MemAvailable:") {
            avail_kb = line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok());
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

fn read_battery() -> Option<String> {
    let entries = fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("");
        if !name.starts_with("BAT") {
            continue;
        }
        let capacity = path.join("capacity");
        if let Ok(contents) = fs::read_to_string(capacity) {
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
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
    let entries = fs::read_dir("/sys/class/net").ok()?;
    let mut fallback = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let iface = path.file_name().and_then(|value| value.to_str()).unwrap_or("lo");
        if iface == "lo" {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(iface.to_string());
        }
        let operstate = fs::read_to_string(path.join("operstate")).unwrap_or_default();
        if operstate.trim() == "up" {
            return Some(iface.to_string());
        }
    }
    fallback
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
