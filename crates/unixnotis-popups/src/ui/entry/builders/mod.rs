//! Kind-specific GTK popup builders

mod common;
mod communication;
mod layout;
mod reply;
mod utility;

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::presentation::{PopupEntryViewModel, PopupKind};
use crate::ui::UiState;

pub(super) use common::{build_action_row, build_close_button};
pub(in crate::ui::entry) use reply::build_inline_reply;

/// Result of building one kind-specific card body
pub(super) struct RenderedPopup {
    pub(super) widget: gtk::Grid,
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
        PopupKind::Utility | PopupKind::Media => {
            utility::build_utility_popup(state, notification, view)
        }
    }
}

pub(super) fn append_thumbnail(
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    content: &gtk::Box,
) -> bool {
    if !should_append_thumbnail(view) {
        return false;
    }
    let image = UiState::build_content_image_widget(notification);
    let Some(image) = image else {
        return false;
    };
    if image.paintable().is_none() {
        return false;
    }

    // Only genuine message media belongs below the body in the content lane
    image.set_halign(gtk::Align::Start);
    image.add_css_class("unixnotis-popup-content-image");
    content.append(&image);
    true
}

const fn should_append_thumbnail(view: &PopupEntryViewModel) -> bool {
    // Sender and application visuals are identity-lane data, never message attachments
    matches!(view.thumbnail, super::presentation::ThumbnailKind::Content)
}

#[cfg(test)]
#[path = "tests/thumbnail.rs"]
mod tests;
