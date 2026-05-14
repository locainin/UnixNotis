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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum BuiltinStatKey {
    // Every CPU card reads the same procfs source
    Cpu,
    // Every memory card reads the same procfs source
    Memory,
    // Load average is shared across cards too
    Load,
    // Battery cards share one aggregated battery snapshot
    Battery,
    // Network cards only share reads when they target the same interface
    Network { iface: Option<String> },
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

    pub(super) fn key(&self) -> BuiltinStatKey {
        match &self.kind {
            BuiltinStatKind::Cpu => BuiltinStatKey::Cpu,
            BuiltinStatKind::Memory => BuiltinStatKey::Memory,
            BuiltinStatKind::Load => BuiltinStatKey::Load,
            BuiltinStatKind::Battery => BuiltinStatKey::Battery,
            BuiltinStatKind::Network { iface } => BuiltinStatKey::Network {
                iface: iface.clone(),
            },
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
#[path = "tests/builtin.rs"]
mod tests;
