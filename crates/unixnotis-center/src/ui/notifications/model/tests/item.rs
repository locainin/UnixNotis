use std::cell::Cell;
use std::rc::Rc;

use gtk::glib::object::ObjectExt;
use unixnotis_core::{NotificationImage, NotificationView};

use super::{RowData, RowItem, RowKind, RowPresentation};

fn notification(id: u32) -> Rc<NotificationView> {
    Rc::new(NotificationView {
        id,
        generation: u64::from(id),
        app_name: "Terminal".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    })
}

#[test]
fn row_data_group_header_sets_expected_fields() {
    let sample = notification(7);
    let data = RowData::group_header(Rc::from("terminal"), 4, true, sample.clone());

    assert_eq!(data.kind, RowKind::GroupHeader);
    assert_eq!(data.count, 4);
    assert!(data.expanded);
    assert!(Rc::ptr_eq(
        data.notification.as_ref().expect("sample"),
        &sample
    ));
}

#[test]
fn row_data_notification_sets_expected_fields() {
    let view = notification(42);
    let presentation = RowPresentation {
        received_at_ms: 123,
        show_metadata: true,
        show_thumbnail: true,
        ..RowPresentation::default()
    };

    let data = RowData::notification(
        Rc::from("terminal"),
        view.clone(),
        true,
        2,
        false,
        true,
        presentation.clone(),
    );

    assert_eq!(data.kind, RowKind::Notification);
    assert_eq!(data.id, 42);
    assert!(data.collapsed_group_preview);
    assert_eq!(data.stack_depth, 2);
    assert!(data.is_active);
    assert_eq!(data.presentation, presentation);
    assert!(Rc::ptr_eq(data.notification.as_ref().expect("view"), &view));
}

#[test]
fn row_item_update_emits_only_for_changed_data() {
    let item = RowItem::new(RowData::notification(
        Rc::from("terminal"),
        notification(1),
        false,
        0,
        false,
        true,
        RowPresentation::default(),
    ));
    let updates = Rc::new(Cell::new(0));
    let updates_clone = updates.clone();
    item.connect_local("updated", false, move |_| {
        updates_clone.set(updates_clone.get() + 1);
        None
    });

    let same = item.data();
    item.update(same);
    assert_eq!(updates.get(), 0);

    item.update(RowData::notification(
        Rc::from("terminal"),
        notification(2),
        false,
        0,
        false,
        true,
        RowPresentation::default(),
    ));
    assert_eq!(updates.get(), 1);
}

#[test]
fn row_data_equivalence_requires_every_rendered_field_to_match() {
    let group = Rc::<str>::from("terminal");
    let view = notification(1);
    let base = RowData::notification(
        group.clone(),
        view.clone(),
        false,
        0,
        false,
        true,
        RowPresentation {
            received_at_ms: 12,
            show_metadata: true,
            show_thumbnail: false,
            ..RowPresentation::default()
        },
    );

    assert!(base.is_equivalent(&base.clone()));

    let mut changed = base.clone();
    changed.kind = RowKind::GroupHeader;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.id = 2;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.group_key = Rc::from("browser");
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.count = 4;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.expanded = true;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.collapsed_group_preview = true;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.stack_depth = 2;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.is_active = false;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.presentation.show_thumbnail = true;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base.clone();
    changed.presentation.reduced_motion = true;
    assert!(!base.is_equivalent(&changed));

    let mut changed = base;
    changed.notification = Some(notification(1));
    assert!(!RowData::notification(
        group,
        view,
        false,
        0,
        false,
        true,
        RowPresentation {
            received_at_ms: 12,
            show_metadata: true,
            show_thumbnail: false,
            ..RowPresentation::default()
        },
    )
    .is_equivalent(&changed));
}

#[test]
fn row_data_same_notification_matches_none_and_shared_rc_only() {
    let left = notification(1);
    let right = notification(1);

    assert!(RowData::same_notification(&None, &None));
    assert!(RowData::same_notification(
        &Some(left.clone()),
        &Some(left.clone())
    ));
    assert!(!RowData::same_notification(&Some(left), &Some(right)));
    assert!(!RowData::same_notification(&Some(notification(2)), &None));
    assert!(!RowData::same_notification(&None, &Some(notification(2))));
}
