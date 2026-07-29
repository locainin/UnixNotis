use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{NotificationImage, NotificationView};

use super::{build_group_row, group_accessible_label, update_group_row};
use crate::control::UiEvent;
use crate::ui::icons::IconResolver;
use crate::ui::notifications::item::{RowData, RowKind};

use crate::ui::notifications::test_support as support;

fn notification(app_name: &str) -> Rc<NotificationView> {
    Rc::new(NotificationView {
        id: 1,
        generation: 1,
        app_name: app_name.to_string(),
        attribution: unixnotis_core::NotificationAttribution::verified(
            app_name,
            app_name,
            "org.example.App",
            "example-app",
            unixnotis_core::AttributionReason::ExactSystemExecutable,
            "exact system executable",
            "system-app:org.example.App".to_string(),
        ),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
    })
}

fn header_button(root: &gtk::Box) -> gtk::Button {
    root.first_child()
        .expect("group row should have button")
        .downcast::<gtk::Button>()
        .expect("group child should be button")
}

fn direct_child_count(container: &gtk::Box) -> usize {
    let mut count = 0;
    let mut child = container.first_child();
    while let Some(widget) = child {
        count += 1;
        child = widget.next_sibling();
    }
    count
}

#[gtk::test]
fn update_group_row_sets_title_count_and_expanded_state() {
    support::init_gtk();
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(4);
    let (root, widgets) = build_group_row(event_tx);
    let data = RowData::group_header(Rc::from("terminal"), 3, false, notification("Terminal"));

    update_group_row(&widgets, &root, &data, &IconResolver::new());

    assert_eq!(widgets.title.text().as_str(), "Terminal");
    assert_eq!(widgets.avatar.width_request(), 26);
    assert_eq!(widgets.avatar.height_request(), 26);
    assert_eq!(widgets.icon.pixel_size(), 18);
    assert_eq!(widgets.count.text().as_str(), "3");
    assert!(gtk::test_accessible_has_property(
        &widgets.button,
        gtk::AccessibleProperty::Label
    ));
    assert_eq!(
        widgets.chevron.icon_name().as_deref(),
        Some("pan-down-symbolic")
    );
    assert!(root.has_css_class("unixnotis-group-row-collapsed"));
    assert!(!root.has_css_class("unixnotis-group-row-expanded"));

    let data = RowData::group_header(Rc::from("terminal"), 4, true, notification("Terminal"));
    update_group_row(&widgets, &root, &data, &IconResolver::new());

    assert_eq!(widgets.count.text().as_str(), "4");
    assert_eq!(
        widgets.chevron.icon_name().as_deref(),
        Some("pan-up-symbolic")
    );
    assert!(!root.has_css_class("unixnotis-group-row-collapsed"));
    assert!(root.has_css_class("unixnotis-group-row-expanded"));
}

#[test]
fn group_accessible_name_keeps_identity_trust_count_and_state() {
    assert_eq!(
        group_accessible_label(
            "Unknown application",
            "Suspicious",
            "Claimed app: Signal",
            4,
            true,
        ),
        "Unknown application. Suspicious. Claimed app: Signal. 4 notifications. Expanded"
    );
    assert_eq!(
        group_accessible_label("Signal", "", "", 1, false),
        "Signal. 1 notification. Collapsed"
    );
}

#[gtk::test]
fn update_group_row_falls_back_to_group_key_without_sample() {
    support::init_gtk();
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(4);
    let (root, widgets) = build_group_row(event_tx);
    let data = RowData {
        kind: RowKind::GroupHeader,
        group_key: Rc::from("terminal"),
        count: 1,
        notification: None,
        ..RowData::default()
    };

    update_group_row(&widgets, &root, &data, &IconResolver::new());

    assert_eq!(widgets.title.text().as_str(), "terminal");
    assert!(!widgets.icon.get_visible());
    assert!(root.has_css_class("unixnotis-group-row-no-icon"));
}

#[gtk::test]
fn update_group_row_keeps_conflict_warning_out_of_the_title() {
    support::init_gtk();
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(4);
    let (root, widgets) = build_group_row(event_tx);
    let mut conflicting = notification("Unknown application").as_ref().clone();
    conflicting.attribution = unixnotis_core::NotificationAttribution::conflict(
        "Trusted Brand",
        "org.example.TrustedBrand",
        unixnotis_core::AttributionReason::ExecutableMismatch,
        "source /tmp/sender-bin",
        "executable:1:2".to_string(),
    );
    let data = RowData::group_header(Rc::from("executable:1:2"), 1, false, Rc::new(conflicting));

    update_group_row(&widgets, &root, &data, &IconResolver::new());

    assert_eq!(widgets.title.text().as_str(), "Unknown application");
    assert!(widgets
        .title
        .tooltip_text()
        .is_some_and(|text| text.contains("/tmp/sender-bin")));
    assert_eq!(
        widgets.icon.icon_name().as_deref(),
        Some("unixnotis-shield-warning-symbolic")
    );
    assert_eq!(
        widgets.secondary.text().as_str(),
        "Claimed app: Trusted Brand"
    );
    assert_eq!(widgets.trust_chip.text().as_str(), "Suspicious");
    assert!(widgets.secondary.get_visible());
    assert!(widgets.trust_chip.get_visible());
    assert!(root.has_css_class("unixnotis-attribution-warning"));
}

#[gtk::test]
fn relay_group_header_keeps_claim_below_command_line_identity() {
    support::init_gtk();
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(4);
    let (root, widgets) = build_group_row(event_tx);
    let mut relayed = notification("Signal").as_ref().clone();
    relayed.attribution = unixnotis_core::NotificationAttribution::relay(
        "Signal",
        "Sent via /usr/bin/notify-send",
        "relay:notify-send:signal".to_string(),
    );
    let data = RowData::group_header(
        Rc::from("relay:notify-send:signal"),
        4,
        false,
        Rc::new(relayed),
    );

    update_group_row(&widgets, &root, &data, &IconResolver::new());

    let header = header_button(&root)
        .child()
        .and_downcast::<gtk::Box>()
        .expect("group header content");
    assert_eq!(direct_child_count(&header), 4);
    assert_eq!(header.spacing(), 8);
    assert_eq!(widgets.title.text().as_str(), "Command-line notification");
    assert_eq!(widgets.secondary.text().as_str(), "App label: Signal");
    assert!(widgets.secondary.get_visible());
    assert!(!widgets.trust_chip.get_visible());
    assert!(root.has_css_class("relay"));
    assert!(!root.has_css_class("unixnotis-attribution-warning"));
}

#[gtk::test]
fn group_header_click_sends_toggle_event() {
    support::init_gtk();
    let (event_tx, event_rx) = async_channel::bounded::<UiEvent>(4);
    let (root, widgets) = build_group_row(event_tx);
    let data = RowData::group_header(Rc::from("terminal"), 2, true, notification("Terminal"));
    update_group_row(&widgets, &root, &data, &IconResolver::new());

    header_button(&root).emit_clicked();

    match event_rx.try_recv().expect("toggle event") {
        UiEvent::GroupToggled(group) => assert_eq!(group, "terminal"),
        event => panic!("expected group toggle event, got {event:?}"),
    }
}
