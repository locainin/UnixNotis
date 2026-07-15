//! Monitor discovery helpers for popup output placement

use gtk::prelude::*;

pub(super) fn find_monitor(output: &str) -> Option<gtk::gdk::Monitor> {
    // No display means monitor discovery cannot proceed
    let display = gtk::gdk::Display::default()?;
    let monitors = display.monitors();

    // Scan all monitors and match by connector/model aliases
    for index in 0..monitors.n_items() {
        let Some(item) = monitors.item(index) else {
            // Sparse monitor lists can return None holes
            continue;
        };
        let Ok(monitor) = item.downcast::<gtk::gdk::Monitor>() else {
            // Ignore non-monitor objects defensively
            continue;
        };
        if monitor_matches_output(&monitor, output) {
            // First match wins to keep output mapping deterministic
            return Some(monitor);
        }
    }

    None
}

fn monitor_matches_output(monitor: &gtk::gdk::Monitor, output: &str) -> bool {
    output_alias_matches(
        monitor.connector().as_deref(),
        monitor.model().as_deref(),
        output,
    )
}

fn output_alias_matches(connector: Option<&str>, model: Option<&str>, output: &str) -> bool {
    let output = output.trim();
    if output.is_empty() {
        // Empty output is treated as no explicit target
        return false;
    }

    // Connector names take priority while model names keep compatibility
    connector.is_some_and(|value| value.eq_ignore_ascii_case(output))
        || model.is_some_and(|value| value.eq_ignore_ascii_case(output))
}

pub(super) fn default_monitor() -> Option<gtk::gdk::Monitor> {
    // Default monitor must degrade gracefully on single-head and edge cases
    let display = gtk::gdk::Display::default()?;
    let monitors = display.monitors();
    let mut best: Option<gtk::gdk::Monitor> = None;
    let mut best_area = 0i64;

    // Prefer the largest monitor as a stable default when no explicit output is set
    for index in 0..monitors.n_items() {
        let Some(item) = monitors.item(index) else {
            continue;
        };
        let Ok(monitor) = item.downcast::<gtk::gdk::Monitor>() else {
            continue;
        };

        // Pixel area is used as a simple deterministic preference heuristic
        let geometry = monitor.geometry();
        let area = i64::from(geometry.width()) * i64::from(geometry.height());
        if area > best_area {
            // Keep largest monitor as default target for popup readability
            best_area = area;
            best = Some(monitor);
        }
    }

    if best.is_some() {
        // Largest-area monitor is used when multiple candidates exist
        return best;
    }

    // Final fallback keeps behavior defined even when geometry probing is sparse
    let item = monitors.item(0)?;
    item.downcast::<gtk::gdk::Monitor>().ok()
}

#[cfg(test)]
#[path = "tests/monitor.rs"]
mod tests;
