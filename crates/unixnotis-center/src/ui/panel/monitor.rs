//! Monitor lookup helpers for panel placement
//!
//! Discovery stays here so window build code only asks for a monitor and applies it

use gtk::gdk;
use gtk::gdk::prelude::*;

pub(super) fn default_monitor() -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    let mut best: Option<gdk::Monitor> = None;
    let mut best_area = 0i64;

    // Pick the largest monitor as a reasonable default when no primary API is available
    for index in 0..monitors.n_items() {
        let Some(item) = monitors.item(index) else {
            continue;
        };
        let Ok(monitor) = item.downcast::<gdk::Monitor>() else {
            continue;
        };
        let geometry = monitor.geometry();
        let area = i64::from(geometry.width()) * i64::from(geometry.height());
        if area > best_area {
            best_area = area;
            best = Some(monitor);
        }
    }

    if best.is_some() {
        return best;
    }

    // Fall back to the first enumerated monitor when discovery yields nothing
    let item = monitors.item(0)?;
    item.downcast::<gdk::Monitor>().ok()
}

pub(super) fn find_monitor(output: &str) -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    for index in 0..monitors.n_items() {
        let Some(item) = monitors.item(index) else {
            continue;
        };
        let Ok(monitor) = item.downcast::<gdk::Monitor>() else {
            continue;
        };
        if monitor_matches_output(&monitor, output) {
            return Some(monitor);
        }
    }
    None
}

fn monitor_matches_output(monitor: &gdk::Monitor, output: &str) -> bool {
    let output = output.trim();
    if output.is_empty() {
        return false;
    }

    let connector = monitor
        .connector()
        .map(|value| value.to_string())
        .unwrap_or_default();
    if !connector.is_empty() && connector.eq_ignore_ascii_case(output) {
        return true;
    }

    let model = monitor
        .model()
        .map(|value| value.to_string())
        .unwrap_or_default();
    if !model.is_empty() && model.eq_ignore_ascii_case(output) {
        return true;
    }

    let manufacturer = monitor
        .manufacturer()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let joined = if manufacturer.is_empty() {
        model
    } else if model.is_empty() {
        manufacturer
    } else {
        format!("{manufacturer} {model}")
    };

    !joined.is_empty() && joined.eq_ignore_ascii_case(output)
}
