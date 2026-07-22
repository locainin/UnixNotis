//! Procfs and sysfs readers for built-in statistic cards

pub(in crate::ui::widgets::stats) mod battery;
mod cpu;
mod dispatch;
mod load;
mod memory;
pub(in crate::ui::widgets::stats) mod network;

pub(super) use battery::read_battery;
pub(super) use cpu::read_cpu_sample;
pub(super) use load::read_loadavg;
pub(super) use memory::read_memory;
pub(super) use network::{extract_iface, read_network};

#[cfg(test)]
mod tests;
