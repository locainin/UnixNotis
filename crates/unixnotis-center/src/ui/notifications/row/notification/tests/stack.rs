use gtk::prelude::*;

use super::{append_stack_layers, stack_layer_visibility, StackLayerVisibility};

#[test]
fn collapsed_stack_depth_maps_to_at_most_two_rear_layers() {
    assert_eq!(stack_layer_visibility(0), StackLayerVisibility::default());
    assert_eq!(
        stack_layer_visibility(1),
        StackLayerVisibility {
            middle: false,
            back: true,
        }
    );
    assert_eq!(
        stack_layer_visibility(2),
        StackLayerVisibility {
            middle: true,
            back: true,
        }
    );
    assert_eq!(stack_layer_visibility(u8::MAX), stack_layer_visibility(2));
}

#[gtk::test]
fn stack_layers_paint_behind_foreground_and_never_accept_input() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let foreground = unixnotis_ui::CutCorner::new(&card, unixnotis_core::CutCorners::default());

    let (middle, back) = append_stack_layers(&root, &foreground);

    assert_eq!(root.first_child().as_ref(), Some(back.upcast_ref()));
    assert_eq!(back.next_sibling().as_ref(), Some(middle.upcast_ref()));
    assert_eq!(
        middle.next_sibling().as_ref(),
        Some(foreground.upcast_ref())
    );
    assert!(!middle.can_target());
    assert!(!back.can_target());
}
