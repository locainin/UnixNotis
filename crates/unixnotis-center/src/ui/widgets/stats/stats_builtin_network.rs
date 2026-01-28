//! Network reader helpers for builtin stats.
//!
//! Handles interface selection, bandwidth sampling, and formatting.

use std::fs;
use std::path::Path;
use std::time::Instant;

use super::BuiltinState;

pub(super) fn read_network(state: &mut BuiltinState, iface: &mut Option<String>) -> Option<String> {
    if iface.is_none() {
        // Choose a stable default interface once to avoid flicker between refreshes.
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
            // Update counters after rate calculation to avoid skew on quick refreshes.
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
pub(super) struct IfaceCandidate {
    // Interface name as reported by sysfs.
    pub(super) name: String,
    // Raw operstate contents ("up", "down", etc), kept for ranking.
    pub(super) operstate: String,
}

pub(super) fn pick_default_iface_from(candidates: &[IfaceCandidate]) -> Option<String> {
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
    // Physical interfaces are favored over virtual ones for default bandwidth stats.
    // This ordering prevents virtual bridges (e.g., docker0) from winning over a
    // real interface that is temporarily down.
    let class_rank = iface_class_rank(candidate.name.as_str());
    // Active interfaces are sorted first within the same class.
    let up_rank = if candidate.operstate.trim() == "up" {
        0
    } else {
        1
    };
    // Name order provides stable ties across runs and reboots.
    (class_rank, up_rank, candidate.name.as_str())
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

pub(super) fn extract_iface(cmd: &str) -> Option<String> {
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
