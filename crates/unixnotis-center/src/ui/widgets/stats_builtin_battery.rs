//! Battery reader helpers for builtin stats.
//!
//! Keeps power_supply parsing isolated so the main builtin stats logic stays
//! focused on selection and state handling.

use std::fs;
use std::path::Path;

pub(super) fn read_battery() -> Option<String> {
    read_battery_from(Path::new("/sys/class/power_supply"))
}

pub(super) fn read_battery_from(root: &Path) -> Option<String> {
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

fn read_power_supply_value(path: &Path) -> Option<u64> {
    // Simple helper that trims and parses numeric power_supply values.
    let contents = fs::read_to_string(path).ok()?;
    contents.trim().parse::<u64>().ok()
}
