use gtk::prelude::*;
use unixnotis_core::CutCorners;

use crate::CutCorner;

#[gtk::test]
fn cut_corner_wraps_one_child_and_retains_configured_geometry() {
    let child = gtk::Label::new(Some("plate"));
    let corners = CutCorners {
        top_left: 12,
        top_right: 8,
        bottom_right: 4,
        bottom_left: 2,
    };

    let wrapper = CutCorner::new(&child, corners);

    assert_eq!(wrapper.child().as_ref(), Some(child.upcast_ref()));
    assert_eq!(wrapper.corners(), corners);
    assert!(wrapper.has_css_class("unixnotis-cut-corner"));
}

#[gtk::test]
fn cut_corner_class_sets_layout_hit_testing_and_cleanup_contracts() {
    let child = gtk::Label::new(Some("plate"));
    let wrapper = CutCorner::new(
        &child,
        CutCorners {
            top_left: 20,
            ..CutCorners::default()
        },
    );
    let window = gtk::Window::new();
    window.set_default_size(100, 60);
    window.set_child(Some(&wrapper));
    window.present();
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }

    assert_eq!(wrapper.css_name(), "unixnotis-cut-corner");
    assert!(wrapper.layout_manager().is_some());
    assert!(!wrapper.contains(1.0, 1.0));
    assert!(wrapper.contains(
        f64::from(wrapper.width()) / 2.0,
        f64::from(wrapper.height()) / 2.0
    ));

    window.set_child(gtk::Widget::NONE);
    window.close();
    drop(window);
    drop(wrapper);
    assert!(child.parent().is_none());
}
