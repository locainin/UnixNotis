//! Collapsed group paint order and bounded rear silhouettes

use gtk::prelude::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct StackLayerVisibility {
    pub(super) middle: bool,
    pub(super) back: bool,
}

pub(super) fn append_stack_layers(
    root: &gtk::Grid,
    foreground: &unixnotis_ui::CutCorner,
) -> (gtk::Box, gtk::Box) {
    let middle = build_stack_layer("unixnotis-stack-layer-middle");
    let back = build_stack_layer("unixnotis-stack-layer-back");

    // A grid measures every visible layer as one ordinary list-row child
    // This keeps positive offsets inside the row allocation
    root.set_hexpand(true);
    root.set_vexpand(false);
    root.set_halign(gtk::Align::Fill);
    root.set_valign(gtk::Align::Start);
    root.attach(&back, 0, 0, 1, 1);
    root.attach(&middle, 0, 0, 1, 1);
    root.attach(foreground, 0, 0, 1, 1);

    // Structural offsets are GTK layout properties, not stylesheet geometry
    back.set_halign(gtk::Align::Fill);
    back.set_valign(gtk::Align::Start);
    back.set_margin_start(20);
    back.set_margin_end(20);
    middle.set_halign(gtk::Align::Fill);
    middle.set_valign(gtk::Align::Start);
    middle.set_margin_top(6);
    middle.set_margin_start(14);
    middle.set_margin_end(14);
    foreground.set_halign(gtk::Align::Fill);
    foreground.set_hexpand(true);
    foreground.set_valign(gtk::Align::Start);
    foreground.set_vexpand(false);
    foreground.set_margin_bottom(0);
    (middle, back)
}

pub(super) fn set_stack_layer_margins(
    foreground: &unixnotis_ui::CutCorner,
    middle: &gtk::Box,
    back: &gtk::Box,
    collapsed: bool,
    grouped: bool,
) {
    // Rear layers keep their measured positive peeks in every row state
    back.set_margin_top(0);
    back.set_margin_start(20);
    back.set_margin_end(20);
    back.set_margin_bottom(0);
    middle.set_margin_top(6);
    middle.set_margin_start(14);
    middle.set_margin_end(14);
    middle.set_margin_bottom(0);

    // Only a collapsed preview needs the foreground's positive inset
    foreground.set_margin_top(if collapsed { 12 } else { 0 });
    foreground.set_margin_start(if grouped { 8 } else { 0 });
    foreground.set_margin_end(if grouped { 8 } else { 0 });
    foreground.set_margin_bottom(0);
}

pub(super) const fn stack_layer_visibility(depth: u8) -> StackLayerVisibility {
    StackLayerVisibility {
        middle: depth >= 2,
        back: depth >= 1,
    }
}

fn build_stack_layer(position_class: &str) -> gtk::Box {
    let layer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layer.add_css_class("unixnotis-stack-layer");
    layer.add_css_class(position_class);
    layer.set_hexpand(true);
    layer.set_halign(gtk::Align::Fill);
    layer.set_vexpand(false);
    layer.set_valign(gtk::Align::Start);
    layer.set_can_target(false);
    layer.set_accessible_role(gtk::AccessibleRole::Presentation);
    layer.set_visible(false);
    layer
}

#[cfg(test)]
#[path = "tests/stack.rs"]
mod tests;
