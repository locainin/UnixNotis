//! Notification list row data and GTK object bindings

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use glib::subclass::prelude::*;
use gtk::glib;
use gtk::glib::object::ObjectExt;
use unixnotis_core::{CutCorners, NotificationMetadataConfig, NotificationView};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    GroupHeader,
    Notification,
}

#[derive(Debug, Clone)]
pub struct RowPresentation {
    // Local receipt timestamp supports relative badges without changing D-Bus payloads
    pub received_at_ms: i64,
    // Optional lanes are disabled by default to preserve the compact stock card
    pub show_metadata: bool,
    pub show_thumbnail: bool,
    // Shared config avoids cloning every metadata string into every row snapshot
    pub metadata: Rc<NotificationMetadataConfig>,
    // Card clipping follows theme reloads through the same row refresh path
    pub card_corners: CutCorners,
}

impl Default for RowPresentation {
    fn default() -> Self {
        Self {
            received_at_ms: 0,
            show_metadata: false,
            show_thumbnail: false,
            metadata: Rc::new(NotificationMetadataConfig::default()),
            card_corners: CutCorners::default(),
        }
    }
}

impl PartialEq for RowPresentation {
    fn eq(&self, other: &Self) -> bool {
        self.received_at_ms == other.received_at_ms
            && self.show_metadata == other.show_metadata
            && self.show_thumbnail == other.show_thumbnail
            && Rc::ptr_eq(&self.metadata, &other.metadata)
            && self.card_corners == other.card_corners
    }
}

impl Eq for RowPresentation {}

#[derive(Debug, Clone)]
pub struct RowData {
    pub kind: RowKind,
    pub id: u32,
    pub group_key: Rc<str>,
    pub count: u32,
    pub expanded: bool,
    // True when this notification is the visible card for a collapsed group
    pub stacked: bool,
    // Number of internal ghost cards shown under the visible notification card
    pub stack_depth: u8,
    pub is_active: bool,
    pub presentation: RowPresentation,
    pub notification: Option<Rc<NotificationView>>,
}

impl Default for RowData {
    fn default() -> Self {
        // Empty notification data is safe for a newly allocated GTK object
        Self {
            kind: RowKind::Notification,
            id: 0,
            group_key: Rc::from(""),
            count: 0,
            expanded: false,
            stacked: false,
            stack_depth: 0,
            is_active: false,
            presentation: RowPresentation::default(),
            notification: None,
        }
    }
}

impl RowData {
    pub fn group_header(
        group_key: Rc<str>,
        count: usize,
        expanded: bool,
        sample: Rc<NotificationView>,
    ) -> Self {
        // Group rows carry one sample only for shared app presentation
        Self {
            kind: RowKind::GroupHeader,
            id: 0,
            group_key,
            count: count as u32,
            expanded,
            stacked: false,
            stack_depth: 0,
            is_active: false,
            presentation: RowPresentation::default(),
            notification: Some(sample),
        }
    }

    pub fn notification(
        group_key: Rc<str>,
        notification: Rc<NotificationView>,
        stacked: bool,
        stack_depth: u8,
        expanded: bool,
        is_active: bool,
        presentation: RowPresentation,
    ) -> Self {
        // Notification rows own the complete immutable render snapshot
        Self {
            kind: RowKind::Notification,
            id: notification.id,
            group_key,
            count: 0,
            expanded,
            stacked,
            stack_depth,
            is_active,
            presentation,
            notification: Some(notification),
        }
    }

    fn is_equivalent(&self, other: &Self) -> bool {
        // Pointer identity avoids deep comparisons of already interned snapshots
        self.kind == other.kind
            && self.id == other.id
            && Rc::ptr_eq(&self.group_key, &other.group_key)
            && self.count == other.count
            && self.expanded == other.expanded
            && self.stacked == other.stacked
            && self.stack_depth == other.stack_depth
            && self.is_active == other.is_active
            && self.presentation == other.presentation
            && Self::same_notification(&self.notification, &other.notification)
    }

    fn same_notification(
        left: &Option<Rc<NotificationView>>,
        right: &Option<Rc<NotificationView>>,
    ) -> bool {
        match (left, right) {
            // Matching shared snapshots guarantee matching notification contents
            (None, None) => true,
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

mod imp {
    use super::{glib, ObjectImpl, ObjectSubclass, OnceLock, RefCell, RowData};

    #[derive(Default)]
    pub struct RowItem {
        pub data: RefCell<RowData>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RowItem {
        const NAME: &'static str = "UnixNotisRowItem";
        type Type = super::RowItem;
    }

    impl ObjectImpl for RowItem {
        fn signals() -> &'static [glib::subclass::Signal] {
            // Signal metadata is allocated once for every row instance
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![glib::subclass::Signal::builder("updated").build()])
        }
    }
}

glib::wrapper! {
    pub struct RowItem(ObjectSubclass<imp::RowItem>);
}

impl RowItem {
    pub fn new(data: RowData) -> Self {
        // Initial data is installed before the object reaches a list binding
        let item: Self = glib::Object::new::<Self>();
        item.imp().data.replace(data);
        item
    }

    pub fn update(&self, data: RowData) {
        // Batch change notifications so row bindings update once per mutation
        let _notify_guard = self.freeze_notify();
        {
            let mut slot = self.imp().data.borrow_mut();
            if slot.is_equivalent(&data) {
                return;
            }
            *slot = data;
        }
        self.emit_by_name::<()>("updated", &[]);
    }

    pub fn data(&self) -> RowData {
        // Callers receive a snapshot so no RefCell borrow crosses GTK callbacks
        self.imp().data.borrow().clone()
    }
}

#[cfg(test)]
#[path = "tests/item.rs"]
mod tests;
