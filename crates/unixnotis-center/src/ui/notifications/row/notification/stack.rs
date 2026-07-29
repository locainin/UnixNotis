//! Collapsed group depth layers and paint order

use gtk::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StackLayer {
    Back,
    Middle,
    Foreground,
}

const STACK_LAYER_ORDER: [StackLayer; 3] =
    [StackLayer::Back, StackLayer::Middle, StackLayer::Foreground];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct StackLayerVisibility {
    pub(super) middle: bool,
    pub(super) back: bool,
}

pub(super) fn append_stack_layers(
    root: &gtk::Box,
    foreground: &unixnotis_ui::CutCorner,
) -> (gtk::Box, gtk::Box) {
    let middle = build_stack_layer("unixnotis-stack-layer-middle");
    let back = build_stack_layer("unixnotis-stack-layer-back");

    // Later GTK siblings paint over earlier layers when negative margins overlap
    for layer in STACK_LAYER_ORDER {
        match layer {
            StackLayer::Back => root.append(&back),
            StackLayer::Middle => root.append(&middle),
            StackLayer::Foreground => root.append(foreground),
        }
    }
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
