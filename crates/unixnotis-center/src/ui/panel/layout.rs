//! Panel sizing, anchoring, and keyboard-mode helpers
//!
//! These rules are shared by initial build and later config reloads

use gtk::gdk;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, LayerShell};
use unixnotis_core::{
    Anchor, Config, Margins, PanelKeyboardInteractivity, PANEL_RUNTIME_WIDTH_MIN,
};

use super::types::PanelWidgets;

// Keep panel width reasonable on narrow displays to avoid dominating screen real estate
const PANEL_WIDTH_MONITOR_RATIO_CAP: f32 = 0.32;
// Width floor keeps controls readable when monitor geometry is very small
const PANEL_WIDTH_MIN: i32 = PANEL_RUNTIME_WIDTH_MIN;

pub fn live_panel_width(root: &gtk::Box) -> i32 {
    // Allocated width is the real live size once GTK has laid the panel out
    let allocated = root.allocated_width();
    if allocated > 0 {
        return allocated;
    }
    // Requested width is only a fallback for early startup and cold rebuild paths
    root.width_request().max(1)
}

pub(super) fn resolve_panel_size(
    config: &Config,
    monitor: Option<&gdk::Monitor>,
    reserved: Option<Margins>,
) -> (i32, i32) {
    // Width is constrained by monitor geometry so defaults stay usable on laptops
    let width = resolve_panel_width(config, monitor);
    let height = resolve_panel_height(config, monitor, reserved).unwrap_or(-1);
    (width, height)
}

pub(super) fn apply_anchor(window: &impl IsA<gtk::Window>, anchor: Anchor, margin: Margins) {
    for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
        window.set_anchor(edge, false);
    }
    match anchor {
        Anchor::TopRight => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::TopLeft => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
        }
        Anchor::BottomRight => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::BottomLeft => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
        }
        Anchor::Top => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::Bottom => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::Left => {
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Top, true);
            // Avoid bottom anchoring so computed height and overrides are respected
        }
        Anchor::Right => {
            window.set_anchor(Edge::Right, true);
            window.set_anchor(Edge::Top, true);
            // Avoid bottom anchoring so computed height and overrides are respected
        }
    }

    window.set_margin(Edge::Top, margin.top);
    window.set_margin(Edge::Right, margin.right);
    window.set_margin(Edge::Bottom, margin.bottom);
    window.set_margin(Edge::Left, margin.left);
}

pub fn apply_panel_config(panel: &PanelWidgets, config: &Config, reserved: Option<Margins>) {
    let monitor = if let Some(output) = config.panel.output.as_ref() {
        super::monitor::find_monitor(output).or_else(super::monitor::default_monitor)
    } else {
        super::monitor::default_monitor()
    };
    if let Some(monitor) = monitor.as_ref() {
        panel.window.set_monitor(Some(monitor));
    }

    panel
        .window
        .set_keyboard_mode(map_keyboard_mode(config.panel.keyboard_interactivity));
    apply_anchor(&panel.window, config.panel.anchor, config.panel.margin);

    let (width, height) = resolve_panel_size(config, monitor.as_ref(), reserved);
    panel.window.set_default_size(width, height);
    if height > 0 {
        panel.window.set_size_request(width, height);
    } else {
        panel.window.set_size_request(width, -1);
    }
    panel.root.set_size_request(width, -1);
    // Child content sits inside theme-controlled padding and optional section
    // margins, so only the outer shell receives an exact width request
}

pub(super) fn map_keyboard_mode(mode: PanelKeyboardInteractivity) -> KeyboardMode {
    match mode {
        PanelKeyboardInteractivity::None => KeyboardMode::None,
        PanelKeyboardInteractivity::OnDemand => KeyboardMode::OnDemand,
        PanelKeyboardInteractivity::Exclusive => KeyboardMode::Exclusive,
    }
}

fn resolve_panel_width(config: &Config, monitor: Option<&gdk::Monitor>) -> i32 {
    let requested = config.panel.width.max(1);
    let Some(monitor) = monitor else {
        return requested;
    };
    let geometry = monitor.geometry();
    let available = geometry.width() - (config.panel.margin.left + config.panel.margin.right);
    if available <= 0 {
        return requested;
    }
    // Ratio cap prevents oversized side panels on compact displays
    let ratio_cap = ((available as f32) * PANEL_WIDTH_MONITOR_RATIO_CAP).round() as i32;
    let max_width = available.max(1);
    let min_width = PANEL_WIDTH_MIN.min(max_width);
    let bounded_cap = ratio_cap.clamp(min_width, max_width);
    requested.min(bounded_cap).max(1)
}

fn resolve_panel_height(
    config: &Config,
    monitor: Option<&gdk::Monitor>,
    reserved: Option<Margins>,
) -> Option<i32> {
    let usable_height = usable_panel_height(config, monitor, reserved);
    if let Some(height_override) = config.panel.height_override {
        // Pixel override is still bounded by the current monitor work area
        return Some(
            usable_height
                .map(|usable| height_override.min(usable))
                .unwrap_or(height_override)
                .max(1),
        );
    }
    let usable_height = usable_height?;
    Some(height_from_percent(usable_height, config.panel.height))
}

fn usable_panel_height(
    config: &Config,
    monitor: Option<&gdk::Monitor>,
    reserved: Option<Margins>,
) -> Option<i32> {
    let monitor = monitor?;
    let geometry = monitor.geometry();
    let mut usable = geometry.height() - (config.panel.margin.top + config.panel.margin.bottom);
    if config.panel.respect_work_area {
        if let Some(reserved) = reserved {
            usable -= reserved.top + reserved.bottom;
        }
    }
    (usable > 0).then_some(usable)
}

fn height_from_percent(usable_height: i32, percent: i32) -> i32 {
    let usable_height = usable_height.max(1);
    let percent = percent.clamp(1, 100);
    let scaled = (i64::from(usable_height) * i64::from(percent) + 50) / 100;
    i32::try_from(scaled).unwrap_or(i32::MAX).max(1)
}

#[cfg(test)]
mod tests {
    use super::height_from_percent;

    #[test]
    fn height_from_percent_scales_usable_height() {
        assert_eq!(height_from_percent(1000, 84), 840);
        assert_eq!(height_from_percent(701, 84), 589);
    }

    #[test]
    fn height_from_percent_keeps_a_positive_floor() {
        assert_eq!(height_from_percent(1, 1), 1);
        assert_eq!(height_from_percent(40, 1), 1);
    }
}
