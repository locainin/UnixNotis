//! CPU reader helpers for builtin stats.
//!
//! Reads /proc/stat and returns total/idle counters for usage calculation.

use std::fs;

pub(super) fn read_cpu_sample() -> Option<(u64, u64)> {
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
