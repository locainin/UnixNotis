use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::EmptyStateAlignment;

use crate::ui::notifications::test_support as support;

#[gtk::test]
fn new_list_attaches_overlay_to_scroller() {
    support::init_gtk();
    let scroller = gtk::ScrolledWindow::new();
    let (command_tx, event_tx) = support::channels();

    let list = crate::ui::notifications::NotificationList::new(
        scroller.clone(),
        command_tx,
        event_tx,
        std::rc::Rc::new(crate::ui::icons::IconResolver::new()),
        support::list_config(),
    );

    assert!(scroller.child().is_some());
    let viewport = scroller
        .child()
        .and_downcast::<gtk::Viewport>()
        .expect("scroller should wrap the notification list in a viewport");
    let overlay = viewport
        .child()
        .and_downcast::<gtk::Overlay>()
        .expect("viewport should contain the notification-list overlay");
    let list_view = overlay
        .child()
        .and_downcast::<gtk::ListView>()
        .expect("overlay should keep the virtualized list as its main child");
    assert_eq!(list_view.margin_bottom(), 0);
    assert_eq!(list.empty_text, "No notifications");
    assert_eq!(list.no_matching_text, "No matching notifications");
    assert_eq!(list.empty_offset_top, 24);
    assert!(list.empty_overlay.get_visible());
}

#[gtk::test]
fn mapped_rows_keep_adjacent_groups_separated_and_stack_inside_allocation() {
    support::init_gtk();
    let scroller = gtk::ScrolledWindow::new();
    let (command_tx, event_tx) = support::channels();
    let mut list = crate::ui::notifications::NotificationList::new(
        scroller.clone(),
        command_tx,
        event_tx,
        std::rc::Rc::new(crate::ui::icons::IconResolver::new()),
        support::list_config(),
    );

    // Two multi-item groups exercise headers, collapsed stacks, and a final row
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
            support::notification(3, "Browser"),
            support::notification(4, "Browser"),
            support::notification(5, "Editor"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();

    let window = gtk::Window::new();
    window.set_default_size(520, 760);
    window.set_child(Some(&scroller));
    window.present();
    let context = gtk::glib::MainContext::default();
    for _ in 0..8 {
        while context.pending() {
            context.iteration(false);
        }
    }

    let viewport = scroller
        .child()
        .and_downcast::<gtk::Viewport>()
        .expect("scroller should expose a viewport");
    let overlay = viewport
        .child()
        .and_downcast::<gtk::Overlay>()
        .expect("viewport should contain the list overlay");
    let list_view = overlay
        .child()
        .and_downcast::<gtk::ListView>()
        .expect("overlay should contain the virtualized list");

    let mut rows = Vec::new();
    let mut child = list_view.first_child();
    while let Some(item) = child {
        rows.push(item);
        child = rows.last().and_then(gtk::prelude::WidgetExt::next_sibling);
    }
    assert!(rows.len() >= 5, "all fixture rows should be mapped");

    for pair in rows.windows(2) {
        let current_item = pair[0]
            .first_child()
            .expect("mapped row should contain its root");
        let current = current_item
            .compute_bounds(&list_view)
            .expect("mapped row should have bounds");
        let next = pair[1]
            .compute_bounds(&list_view)
            .expect("mapped row should have bounds");
        assert!(
            next.y() >= current.y() + current.height() + 7.5,
            "adjacent ListView rows need the explicit 8px gap: current={current:?}, next={next:?}"
        );
    }

    for item in rows {
        let Some(root) = item.first_child() else {
            continue;
        };
        let Some(stack) = root.first_child().and_downcast::<gtk::Grid>() else {
            continue;
        };
        let stack_bounds = stack
            .compute_bounds(&root)
            .expect("notification stack should have bounds");
        let root_bounds = root
            .compute_bounds(&item)
            .expect("notification row should have bounds");
        assert!(stack_bounds.y() >= 0.0);
        assert!(
            stack_bounds.y() + stack_bounds.height() <= root_bounds.height(),
            "stack must remain inside its real ListView row allocation"
        );
    }

    window.close();
}

#[gtk::test]
fn mapped_notification_foregrounds_fill_every_stack_mode() {
    support::init_gtk();
    let scroller = gtk::ScrolledWindow::new();
    let (command_tx, event_tx) = support::channels();
    let mut list = crate::ui::notifications::NotificationList::new(
        scroller.clone(),
        command_tx,
        event_tx,
        std::rc::Rc::new(crate::ui::icons::IconResolver::new()),
        support::list_config(),
    );

    // Two groups cover depth one, depth two, and a standalone row
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
            support::notification(3, "Browser"),
            support::notification(4, "Browser"),
            support::notification(5, "Browser"),
            support::notification(6, "Editor"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();

    let window = gtk::Window::new();
    window.set_default_size(620, 900);
    window.set_child(Some(&scroller));
    window.present();
    pump_gtk_frames();

    let list_view = mapped_list_view(&scroller);
    assert_foregrounds_fill_rows(&list_view, 3);

    // Rebuild the same model with an expanded group so foreground width does not
    // depend on a visible rear layer from the previous binding
    list.toggle_group("test:Browser");
    list.flush_rebuild();
    pump_gtk_frames();
    assert_foregrounds_fill_rows(&list_view, 5);

    // Return to a collapsed state to exercise another recycled-row transition
    list.toggle_group("test:Browser");
    list.flush_rebuild();
    pump_gtk_frames();
    assert_foregrounds_fill_rows(&list_view, 3);

    window.close();
}

fn pump_gtk_frames() {
    let context = gtk::glib::MainContext::default();
    for _ in 0..8 {
        while context.pending() {
            context.iteration(false);
        }
    }
}

fn mapped_list_view(scroller: &gtk::ScrolledWindow) -> gtk::ListView {
    let viewport = scroller
        .child()
        .and_downcast::<gtk::Viewport>()
        .expect("scroller should expose a viewport");
    viewport
        .child()
        .and_downcast::<gtk::Overlay>()
        .expect("viewport should contain the list overlay")
        .child()
        .and_downcast::<gtk::ListView>()
        .expect("overlay should contain the virtualized list")
}

fn assert_foregrounds_fill_rows(list_view: &gtk::ListView, minimum_rows: usize) {
    let mut child = list_view.first_child();
    let mut notification_rows = 0;
    while let Some(item) = child {
        let Some(root) = item.first_child() else {
            child = item.next_sibling();
            continue;
        };
        let Some(stack) = root.first_child().and_downcast::<gtk::Grid>() else {
            child = item.next_sibling();
            continue;
        };
        let Some(foreground) = stack.last_child().and_downcast::<unixnotis_ui::CutCorner>() else {
            panic!("notification grid should end with its foreground card");
        };
        let grouped_inset =
            if foreground.has_css_class(unixnotis_core::css::hooks::panel_card::GROUPED) {
                8
            } else {
                0
            };
        let expected = stack.width() - (grouped_inset * 2);
        assert!(
            foreground.width() >= expected - 1,
            "foreground width {} did not fill stack width {} with inset {}",
            foreground.width(),
            stack.width(),
            grouped_inset
        );
        assert_eq!(foreground.halign(), gtk::Align::Fill);
        assert!(foreground.hexpands());
        notification_rows += 1;
        child = item.next_sibling();
    }
    assert!(
        notification_rows >= minimum_rows,
        "mapped fixture should include collapsed, expanded, and standalone notifications"
    );
}

#[gtk::test]
fn apply_config_updates_empty_copy_and_offset() {
    let mut list = support::make_list();
    let mut config = support::list_config();
    config.empty_text = "All clear".to_string();
    config.no_matching_text = "Nothing found".to_string();
    config.empty_offset_top = 48;

    list.apply_config(&config);
    list.set_empty_layout(true);

    assert_eq!(list.empty_text, "All clear");
    assert_eq!(list.no_matching_text, "Nothing found");
    assert_eq!(list.empty_offset_top, 48);
    assert_eq!(list.empty_overlay.margin_top(), 48);
}

#[gtk::test]
fn apply_config_requests_rebuild_when_metadata_or_thumbnail_flags_change() {
    let mut list = support::make_list();
    let mut config = support::list_config();
    config.show_notification_metadata = true;

    list.apply_config(&config);

    assert!(list.show_notification_metadata);
    assert!(!list.show_notification_thumbnails);
    assert!(list.needs_rebuild());

    list.needs_rebuild = false;
    config.show_notification_thumbnails = true;

    list.apply_config(&config);

    assert!(list.show_notification_metadata);
    assert!(list.show_notification_thumbnails);
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn apply_config_requests_rebuild_when_metadata_text_or_corner_geometry_changes() {
    let mut list = support::make_list();
    let mut config = support::list_config();
    config.notification_metadata.live_label = "CURRENT".to_string();

    list.apply_config(&config);

    assert_eq!(list.notification_metadata.live_label, "CURRENT");
    assert!(list.needs_rebuild());

    list.needs_rebuild = false;
    config.notification_corners.top_right = 16;
    list.apply_config(&config);

    assert_eq!(list.notification_corners.top_right, 16);
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn apply_config_refreshes_existing_rows_when_reduced_motion_changes() {
    let mut list = support::make_list();
    list.seed(vec![support::notification(1, "Terminal")], Vec::new());
    list.flush_rebuild();
    let mut config = support::list_config();
    config.reduced_motion = true;

    list.apply_config(&config);
    list.flush_rebuild();

    let row = list.entries.get(&1).expect("notification should remain");
    assert!(row.item.data().presentation.reduced_motion);
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

#[gtk::test]
fn explicit_empty_alignment_overrides_widget_dependent_default() {
    let mut list = support::make_list();
    let mut config = support::list_config();
    config.empty_alignment = EmptyStateAlignment::End;

    list.apply_config(&config);
    list.set_empty_layout(true);

    assert_eq!(list.empty_overlay.valign(), Align::End);
    assert_eq!(list.empty_overlay.margin_top(), 0);

    config.empty_alignment = EmptyStateAlignment::Start;
    config.empty_offset_top = 36;
    list.apply_config(&config);
    list.set_empty_layout(false);

    assert_eq!(list.empty_overlay.valign(), Align::Start);
    assert_eq!(list.empty_overlay.margin_top(), 36);
}
