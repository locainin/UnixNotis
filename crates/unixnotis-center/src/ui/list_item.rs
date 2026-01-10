//! Notification list row data and GTK object bindings.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use glib::subclass::prelude::*;
use gtk::glib;
use gtk::glib::object::ObjectExt;
use unixnotis_core::NotificationView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    GroupHeader,
    Notification,
    Ghost,
}

#[derive(Debug, Clone)]
pub struct RowData {
    pub kind: RowKind,
    pub id: u32,
    pub group_key: Rc<str>,
    pub count: u32,
    pub expanded: bool,
    pub stacked: bool,
    pub ghost_depth: u8,
    pub is_active: bool,
    pub notification: Option<Rc<NotificationView>>,
}

impl Default for RowData {
    fn default() -> Self {
        Self {
            kind: RowKind::Ghost,
            id: 0,
            group_key: Rc::from(""),
            count: 0,
            expanded: false,
            stacked: false,
            ghost_depth: 0,
            is_active: false,
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
        Self {
            kind: RowKind::GroupHeader,
            id: 0,
            group_key,
            count: count as u32,
            expanded,
            stacked: false,
            ghost_depth: 0,
            is_active: false,
            notification: Some(sample),
        }
    }

    pub fn notification(
        group_key: Rc<str>,
        notification: Rc<NotificationView>,
        stacked: bool,
        is_active: bool,
    ) -> Self {
        Self {
            kind: RowKind::Notification,
            id: notification.id,
            group_key,
            count: 0,
            expanded: false,
            stacked,
            ghost_depth: 0,
            is_active,
            notification: Some(notification),
        }
    }

    pub fn ghost(group_key: Rc<str>, depth: u8) -> Self {
        Self {
            kind: RowKind::Ghost,
            id: 0,
            group_key,
            count: 0,
            expanded: false,
            stacked: false,
            ghost_depth: depth,
            is_active: false,
            notification: None,
        }
    }

    fn is_equivalent(&self, other: &RowData) -> bool {
        self.kind == other.kind
            && self.id == other.id
            && Rc::ptr_eq(&self.group_key, &other.group_key)
            && self.count == other.count
            && self.expanded == other.expanded
            && self.stacked == other.stacked
            && self.ghost_depth == other.ghost_depth
            && self.is_active == other.is_active
            && Self::same_notification(&self.notification, &other.notification)
    }

    fn same_notification(
        left: &Option<Rc<NotificationView>>,
        right: &Option<Rc<NotificationView>>,
    ) -> bool {
        match (left, right) {
            (None, None) => true,
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

mod imp {
    use super::*;

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
        let item: Self = glib::Object::new::<Self>();
        item.imp().data.replace(data);
        item
    }

    pub fn update(&self, data: RowData) {
        // Batch change notifications so row bindings update once per mutation.
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
        self.imp().data.borrow().clone()
    }
}
