//! Memory reader helpers for builtin stats.
//!
//! Parses /proc/meminfo and formats used/total in GiB.

use std::fs;

pub(super) fn read_memory() -> Option<String> {
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
