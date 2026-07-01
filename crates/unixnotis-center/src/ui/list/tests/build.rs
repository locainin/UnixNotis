use gtk::prelude::WidgetExt;
use gtk::Align;

use crate::ui::list::test_support as support;

#[gtk::test]
fn new_list_attaches_overlay_to_scroller() {
    support::init_gtk();
    let scroller = gtk::ScrolledWindow::new();
    let (command_tx, event_tx) = support::channels();

    let list = crate::ui::list::NotificationList::new(
        scroller.clone(),
        command_tx,
        event_tx,
        std::rc::Rc::new(crate::ui::icons::IconResolver::new()),
        support::list_config(),
    );

    assert!(scroller.child().is_some());
    assert_eq!(list.empty_text, "No notifications");
    assert_eq!(list.empty_offset_top, 24);
    assert!(list.empty_overlay.get_visible());
}

#[gtk::test]
fn apply_config_updates_empty_copy_and_offset() {
    let mut list = support::make_list();
    let mut config = support::list_config();
    config.empty_text = "All clear".to_string();
    config.empty_offset_top = 48;

    list.apply_config(&config, true);

    assert_eq!(list.empty_text, "All clear");
    assert_eq!(list.empty_offset_top, 48);
    assert_eq!(list.empty_overlay.margin_top(), 48);
}

#[gtk::test]
fn apply_config_requests_rebuild_when_metadata_or_thumbnail_flags_change() {
    let mut list = support::make_list();
    let mut config = support::list_config();
    config.show_notification_metadata = true;

    list.apply_config(&config, true);

    assert!(list.show_notification_metadata);
    assert!(!list.show_notification_thumbnails);
    assert!(list.needs_rebuild());

    list.needs_rebuild = false;
    config.show_notification_thumbnails = true;

    list.apply_config(&config, true);

    assert!(list.show_notification_metadata);
    assert!(list.show_notification_thumbnails);
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn set_empty_layout_switches_between_widget_offset_and_centered_empty_state() {
    let list = support::make_list();

    list.empty_overlay.set_valign(Align::Center);
    list.empty_overlay.set_margin_top(0);
    list.set_empty_layout(true);

    assert_eq!(list.empty_overlay.valign(), Align::Start);
    assert_eq!(list.empty_overlay.margin_top(), list.empty_offset_top);

    list.set_empty_layout(false);

    assert_eq!(list.empty_overlay.valign(), Align::Center);
    assert_eq!(list.empty_overlay.margin_top(), 0);
}
