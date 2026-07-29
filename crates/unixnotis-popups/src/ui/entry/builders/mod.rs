//! Kind-specific GTK popup builders

mod common;
mod communication;
mod reply;
mod utility;
mod warning;

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::presentation::{PopupEntryViewModel, PopupKind};
use crate::ui::UiState;

pub(super) use common::{build_action_row, build_close_button};
pub(in crate::ui::entry) use reply::build_inline_reply;

/// Result of building one kind-specific card body
pub(super) struct RenderedPopup {
    pub(super) widget: gtk::Box,
    pub(super) has_icon: bool,
    pub(super) has_image: bool,
}

pub(super) fn build_popup_content(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
) -> RenderedPopup {
    // Each layout owns its structure so future changes do not grow one conditional builder
    match view.kind {
        PopupKind::Communication => {
            communication::build_communication_popup(state, notification, view)
        }
        PopupKind::Utility => utility::build_utility_popup(state, notification, view),
        PopupKind::Warning => warning::build_warning_popup(state, notification, view),
    }
}

pub(super) fn append_thumbnail(
    state: &UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    content: &gtk::Box,
) -> bool {
    if view.thumbnail != super::presentation::ThumbnailKind::Content {
        return false;
    }
    let Some(image) = state.build_content_image_widget(notification) else {
        return false;
    };

    // Content images stay bounded and visually separate from the application badge
    image.set_halign(gtk::Align::Start);
    image.add_css_class("unixnotis-popup-content-image");
    content.append(&image);
    true
}
