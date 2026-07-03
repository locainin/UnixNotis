use gtk::glib::object::Cast;
use gtk::prelude::{ListItemExt, WidgetExt};
use std::rc::Rc;

use super::{bind_row, ensure_row_widgets, get_row_widgets, set_row_widgets, RowWidgets};
use crate::ui::icons::IconResolver;
use crate::ui::list::item::{RowData, RowItem, RowKind, RowPresentation};

use crate::ui::list::test_support as support;

fn new_gtk_item() -> gtk::ListItem {
    gtk::glib::Object::new::<gtk::ListItem>()
}

fn contains_label_text(root: &gtk::Widget, text: &str) -> bool {
    if root
        .downcast_ref::<gtk::Label>()
        .map(|label| label.text().as_str() == text)
        .unwrap_or(false)
    {
        return true;
    }

    let mut child = root.first_child();
    while let Some(widget) = child {
        if contains_label_text(&widget, text) {
            return true;
        }
        child = widget.next_sibling();
    }
    false
}

#[gtk::test]
fn set_and_get_row_widgets_round_trips_cached_bundle() {
    support::init_gtk();
    let (command_tx, event_tx) = support::channels();
    let gtk_item = new_gtk_item();
    let widgets = std::rc::Rc::new(RowWidgets::new(RowKind::Notification, command_tx, event_tx));

    set_row_widgets(&gtk_item, widgets.clone());

    let cached = get_row_widgets(&gtk_item).expect("widgets should be cached");
    assert!(std::rc::Rc::ptr_eq(&cached, &widgets));
    assert!(gtk_item.child().is_some());
}

#[gtk::test]
fn ensure_row_widgets_reuses_same_kind() {
    support::init_gtk();
    let (command_tx, event_tx) = support::channels();
    let gtk_item = new_gtk_item();

    let first = ensure_row_widgets(
        &gtk_item,
        RowKind::Notification,
        command_tx.clone(),
        event_tx.clone(),
    );
    let second = ensure_row_widgets(&gtk_item, RowKind::Notification, command_tx, event_tx);

    assert!(std::rc::Rc::ptr_eq(&first, &second));
}

#[gtk::test]
fn ensure_row_widgets_replaces_different_kind() {
    support::init_gtk();
    let (command_tx, event_tx) = support::channels();
    let gtk_item = new_gtk_item();

    let first = ensure_row_widgets(
        &gtk_item,
        RowKind::Notification,
        command_tx.clone(),
        event_tx.clone(),
    );
    let second = ensure_row_widgets(&gtk_item, RowKind::GroupHeader, command_tx, event_tx);

    assert!(!std::rc::Rc::ptr_eq(&first, &second));
}

#[gtk::test]
fn bind_row_refreshes_notification_widget_and_tracks_item_updates() {
    support::init_gtk();
    let (command_tx, event_tx) = support::channels();
    let widgets = Rc::new(RowWidgets::new(RowKind::Notification, command_tx, event_tx));
    let notification = Rc::new(support::notification(1, "Terminal"));
    let item = RowItem::new(RowData::notification(
        Rc::from("terminal"),
        notification,
        false,
        0,
        false,
        true,
        RowPresentation::default(),
    ));

    bind_row(
        widgets.clone(),
        &item,
        &item.data(),
        Rc::new(IconResolver::new()),
    );

    assert!(contains_label_text(
        &widgets.root.clone().upcast::<gtk::Widget>(),
        "summary 1"
    ));

    let changed = Rc::new(support::notification(2, "Terminal"));
    item.update(RowData::notification(
        Rc::from("terminal"),
        changed,
        false,
        0,
        false,
        true,
        RowPresentation::default(),
    ));

    assert!(contains_label_text(
        &widgets.root.clone().upcast::<gtk::Widget>(),
        "summary 2"
    ));
}

#[gtk::test]
fn unbind_disconnects_row_item_update_handler() {
    support::init_gtk();
    let (command_tx, event_tx) = support::channels();
    let widgets = Rc::new(RowWidgets::new(RowKind::Notification, command_tx, event_tx));
    let notification = Rc::new(support::notification(1, "Terminal"));
    let item = RowItem::new(RowData::notification(
        Rc::from("terminal"),
        notification,
        false,
        0,
        false,
        true,
        RowPresentation::default(),
    ));
    bind_row(
        widgets.clone(),
        &item,
        &item.data(),
        Rc::new(IconResolver::new()),
    );
    assert!(contains_label_text(
        &widgets.root.clone().upcast::<gtk::Widget>(),
        "summary 1"
    ));

    widgets.unbind();
    let changed = Rc::new(support::notification(2, "Terminal"));
    item.update(RowData::notification(
        Rc::from("terminal"),
        changed,
        false,
        0,
        false,
        true,
        RowPresentation::default(),
    ));

    assert!(contains_label_text(
        &widgets.root.clone().upcast::<gtk::Widget>(),
        "summary 1"
    ));
    assert!(!contains_label_text(
        &widgets.root.clone().upcast::<gtk::Widget>(),
        "summary 2"
    ));
}
