//! Panel motion preference tests

use gtk::prelude::*;
use unixnotis_core::css::hooks;

use super::apply_motion_class;

#[gtk::test]
fn reduced_motion_class_tracks_the_runtime_preference() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

    apply_motion_class(&root, true);
    assert!(root.has_css_class(hooks::panel_shell::REDUCED_MOTION));

    apply_motion_class(&root, false);
    assert!(!root.has_css_class(hooks::panel_shell::REDUCED_MOTION));
}
