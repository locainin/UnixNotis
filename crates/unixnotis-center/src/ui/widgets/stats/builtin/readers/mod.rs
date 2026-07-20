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
