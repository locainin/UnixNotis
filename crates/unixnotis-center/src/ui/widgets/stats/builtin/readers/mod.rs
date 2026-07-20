//! Procfs and sysfs readers for built-in statistic cards

pub(super) mod battery;
mod cpu;
mod load;
mod memory;
pub(super) mod network;

pub(super) use battery::read_battery;
pub(super) use cpu::read_cpu_sample;
pub(super) use load::read_loadavg;
pub(super) use memory::read_memory;
pub(super) use network::{extract_iface, read_network};

use super::model::{BuiltinStat, BuiltinStatKind, BuiltinState};

impl BuiltinStat {
    pub(in crate::ui::widgets::stats) fn read(&mut self) -> Option<String> {
        match &mut self.kind {
            BuiltinStatKind::Cpu => self.read_cpu(),
            BuiltinStatKind::Memory => read_memory(),
            BuiltinStatKind::Load => read_loadavg(),
            BuiltinStatKind::Battery => read_battery(),
            BuiltinStatKind::Network { iface } => read_network(&mut self.state, iface),
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
                    // Delta-based usage avoids spikes when the counter wraps
                    let delta_total = total - *last_total;
                    let delta_idle = idle.saturating_sub(*last_idle);
                    100.0 * (delta_total.saturating_sub(delta_idle)) as f64 / delta_total as f64
                } else if total > 0 {
                    // First read falls back to absolute usage
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
