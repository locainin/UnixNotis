//! Collapsed group paint order and bounded rear silhouettes

use gtk::prelude::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct StackLayerVisibility {
    pub(super) middle: bool,
    pub(super) back: bool,
}

pub(super) fn append_stack_layers(
    root: &gtk::Overlay,
    foreground: &unixnotis_ui::CutCorner,
) -> (gtk::Box, gtk::Box) {
    let middle = build_stack_layer("unixnotis-stack-layer-middle");
    let back = build_stack_layer("unixnotis-stack-layer-back");

    // One overlay allocation keeps all three layers in the same measured cell
    root.set_child(Some(&back));
    root.add_overlay(&middle);
    root.add_overlay(foreground);

    // Rear shells never determine row height; the readable card does
    root.set_measure_overlay(&middle, false);
    root.set_measure_overlay(foreground, true);
    (middle, back)
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
    layer.set_can_target(false);
    layer.set_accessible_role(gtk::AccessibleRole::Presentation);
    layer.set_visible(false);
    layer
}

#[cfg(test)]
#[path = "tests/stack.rs"]
mod tests;
