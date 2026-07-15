//! Anchor and margin helpers for popup layer-shell windows

use gtk::prelude::*;
use gtk4_layer_shell::{Edge, LayerShell};
use unixnotis_core::{Anchor, Margins};

pub(super) fn apply_anchor(window: &impl IsA<gtk::Window>, anchor: Anchor, margin: Margins) {
    // Reset all edges first so anchor changes never leave stale flags behind
    for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
        window.set_anchor(edge, false);
    }

    let [top, right, bottom, left] = anchor_flags(anchor);
    for (edge, enabled) in [
        (Edge::Top, top),
        (Edge::Right, right),
        (Edge::Bottom, bottom),
        (Edge::Left, left),
    ] {
        // Apply the complete edge plan after clearing stale anchor state
        window.set_anchor(edge, enabled);
    }

    // Margins are always applied after anchor selection
    window.set_margin(Edge::Top, margin.top);
    window.set_margin(Edge::Right, margin.right);
    window.set_margin(Edge::Bottom, margin.bottom);
    window.set_margin(Edge::Left, margin.left);
}

const fn anchor_flags(anchor: Anchor) -> [bool; 4] {
    // Order stays top, right, bottom, left to match layer-shell edge handling
    match anchor {
        Anchor::TopRight => [true, true, false, false],
        Anchor::TopLeft => [true, false, false, true],
        Anchor::BottomRight => [false, true, true, false],
        Anchor::BottomLeft => [false, false, true, true],
        Anchor::Top => [true, true, false, true],
        Anchor::Bottom => [false, true, true, true],
        Anchor::Left => [true, false, true, true],
        Anchor::Right => [true, true, true, false],
    }
}

#[cfg(test)]
#[path = "tests/anchor.rs"]
mod tests;
