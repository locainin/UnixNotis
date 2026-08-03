use gtk::prelude::*;

use super::{
    append_stack_layers, set_stack_layer_margins, stack_layer_visibility, StackLayerVisibility,
};

#[test]
fn collapsed_stack_depth_maps_to_two_rear_layers() {
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
    let root = gtk::Grid::new();
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let foreground = unixnotis_ui::CutCorner::new(&card, unixnotis_core::CutCorners::default());

    let (middle, back) = append_stack_layers(&root, &foreground);
    assert_eq!(back.margin_start(), 20);
    assert_eq!(middle.margin_top(), 6);
    assert_eq!(middle.margin_start(), 14);

    assert_eq!(root.child_at(0, 0).as_ref(), Some(back.upcast_ref()));
    assert_eq!(
        root.child_at(0, 0)
            .expect("back layer should be attached")
            .next_sibling()
            .as_ref(),
        Some(middle.upcast_ref())
    );
    assert_eq!(
        middle.next_sibling().as_ref(),
        Some(foreground.upcast_ref())
    );
    assert!(!root.vexpands());
    assert_eq!(root.valign(), gtk::Align::Start);
    assert!(!middle.can_target());
    assert!(!back.can_target());
}

#[gtk::test]
fn measured_stack_includes_visible_positive_offset_layers() {
    let root = gtk::Grid::new();
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.set_size_request(-1, 100);
    let foreground = unixnotis_ui::CutCorner::new(&card, unixnotis_core::CutCorners::default());

    let (middle, back) = append_stack_layers(&root, &foreground);
    set_stack_layer_margins(&foreground, &middle, &back, true, true);
    middle.set_size_request(-1, 68);
    back.set_size_request(-1, 68);
    middle.set_visible(true);
    back.set_visible(true);
    foreground.set_visible(true);

    let (_, natural_height, _, _) = root.measure(gtk::Orientation::Vertical, 320);

    assert!(
        natural_height >= 112,
        "measured grid height {natural_height} must contain the foreground offset"
    );

    let allocation = gtk::Allocation::new(0, 0, 320, natural_height);
    root.size_allocate(&allocation, -1);

    let layers: [&gtk::Widget; 3] = [
        back.upcast_ref::<gtk::Widget>(),
        middle.upcast_ref::<gtk::Widget>(),
        foreground.upcast_ref::<gtk::Widget>(),
    ];
    for layer in layers {
        if !layer.is_visible() {
            continue;
        }
        let layer_bounds = layer
            .compute_bounds(&root)
            .expect("visible stack layers should have bounds");
        assert!(layer_bounds.y() >= 0.0);
        assert!(
            layer_bounds.y() + layer_bounds.height() <= natural_height as f32,
            "visible stack layer must remain within its measured row"
        );
    }

    let foreground_y = foreground
        .compute_bounds(&root)
        .expect("foreground should have bounds")
        .y();
    let foreground_width = foreground.width();
    assert_eq!(foreground_width, 304);
    let middle_y = middle
        .compute_bounds(&root)
        .expect("middle layer should have bounds")
        .y();
    let back_y = back
        .compute_bounds(&root)
        .expect("back layer should have bounds")
        .y();
    assert!(foreground_y >= middle_y);
    assert!(middle_y >= back_y);
}

#[gtk::test]
fn foreground_fills_grid_when_rear_layers_are_hidden() {
    let root = gtk::Grid::new();
    root.set_hexpand(true);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.set_size_request(-1, 80);
    card.set_hexpand(true);
    card.set_halign(gtk::Align::Fill);
    let foreground = unixnotis_ui::CutCorner::new(&card, unixnotis_core::CutCorners::default());

    let (middle, back) = append_stack_layers(&root, &foreground);
    set_stack_layer_margins(&foreground, &middle, &back, false, false);
    foreground.set_visible(true);

    let allocation = gtk::Allocation::new(0, 0, 320, 80);
    root.size_allocate(&allocation, -1);

    assert_eq!(root.width(), 320);
    assert_eq!(foreground.width(), 320);
    assert_eq!(card.width(), 320);
}
