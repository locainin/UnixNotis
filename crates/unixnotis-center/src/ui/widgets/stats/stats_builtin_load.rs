//! Load average reader helpers for builtin stats.
//!
//! Keeps /proc/loadavg parsing separate from builtin state handling.

use std::fs;

pub(super) fn read_loadavg() -> Option<String> {
    let contents = fs::read_to_string("/proc/loadavg").ok()?;
    let mut parts = contents.split_whitespace();
    let one = parts.next()?;
    let five = parts.next()?;
    let fifteen = parts.next()?;
    Some(format!("{} {} {}", one, five, fifteen))
}
