use gtk::prelude::*;

use super::{build_empty_row, update_empty_row};

use crate::ui::notifications::test_support as support;

fn empty_label(root: &gtk::Box) -> gtk::Label {
    root.first_child()
        .expect("empty row should have label")
        .downcast::<gtk::Label>()
        .expect("child should be label")
}

#[gtk::test]
fn build_empty_row_sets_layout_and_text() {
    support::init_gtk();

    let root = build_empty_row("Nothing here");
    let label = empty_label(&root);

    assert!(root.has_css_class("unixnotis-empty"));
    assert!(label.has_css_class("unixnotis-empty-label"));
    assert_eq!(label.text().as_str(), "Nothing here");
    assert_eq!(root.valign(), gtk::Align::Center);
}

#[gtk::test]
fn update_empty_row_changes_label_text() {
    support::init_gtk();
    let root = build_empty_row("Nothing here");

    update_empty_row(&root, "All clear");

    assert_eq!(empty_label(&root).text().as_str(), "All clear");
}

#[gtk::test]
fn update_empty_row_tolerates_missing_label() {
    support::init_gtk();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

    update_empty_row(&root, "All clear");

    assert!(root.first_child().is_none());
}
