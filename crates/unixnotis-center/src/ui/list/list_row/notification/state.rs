//! Shared notification-row state and limits
//!
//! This file keeps the reusable row widgets and small helper structs together

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use unixnotis_core::NotificationView;

pub(in crate::ui::list) struct NotificationRowWidgets {
    // Main icon shown at the top-left of the row
    pub(super) icon: gtk::Image,
    // App name text shown beside the icon
    pub(super) app_label: gtk::Label,
    // Summary line with stronger visual weight
    pub(super) summary_label: gtk::Label,
    // Body text section that can span multiple lines
    pub(super) body_label: gtk::Label,
    // Container for optional action buttons
    pub(super) actions_box: gtk::Box,
    // Current notification id bound to this reused row widget
    pub(super) notify_id: Rc<Cell<u32>>,
    // Last rendered action signature for cheap no-op detection
    pub(super) action_cache: RefCell<Vec<(String, String)>>,
    // Last rendered icon signature so decode work only happens on a real change
    pub(super) icon_sig: RefCell<Option<IconSignature>>,
}

// Hard caps keep very large payloads from blowing up row height
pub(super) const MAX_SUMMARY_LABEL_CHARS: usize = 160;
pub(super) const MAX_BODY_LABEL_CHARS: usize = 512;
// Action labels stay bounded so one button cannot distort the whole row
pub(super) const MAX_ACTION_LABEL_CHARS: usize = 20;

pub(super) struct OptionalLabelState<'a> {
    // Hidden rows should collapse instead of leaving dead card spacing
    pub(super) visible: bool,
    // Borrow when possible so repeated row refreshes do not allocate
    pub(super) text: Cow<'a, str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ui::list) struct IconSignature {
    // These fields match the icon resolution inputs
    // If none of them change, the existing paintable is still valid
    image_path: String,
    icon_name: String,
    app_name: String,
    has_image_data: bool,
    image_len: usize,
    image_width: i32,
    image_height: i32,
}

impl IconSignature {
    pub(super) fn from(notification: &NotificationView) -> Self {
        // Signature includes all fields that can change icon resolution output
        // This keeps row refreshes cheap when only text or actions changed
        Self {
            image_path: notification.image.image_path.clone(),
            icon_name: notification.image.icon_name.clone(),
            app_name: notification.app_name.clone(),
            has_image_data: notification.image.has_image_data,
            image_len: notification.image.image_data.data.len(),
            image_width: notification.image.image_data.width,
            image_height: notification.image.image_data.height,
        }
    }
}
