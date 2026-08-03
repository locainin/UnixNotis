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
use unixnotis_ui::presentation::SenderVisualPresentation;

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
    let sender_visual = view.visuals.sender;
    let is_application_visual = sender_visual == SenderVisualPresentation::ApplicationProvidedIcon;
    if view.thumbnail != super::presentation::ThumbnailKind::Content && !is_application_visual {
        return false;
    }
    let image =
        if is_application_visual && view.thumbnail != super::presentation::ThumbnailKind::Content {
            UiState::build_sender_visual_widget(notification)
        } else {
            UiState::build_content_image_widget(notification)
        };
    let Some(image) = image else {
        return false;
    };
    if image.paintable().is_none() {
        return false;
    }

    // Content images stay bounded and visually separate from the application badge
    image.set_halign(gtk::Align::Start);
    image.add_css_class("unixnotis-popup-content-image");
    content.append(&image);
    true
}
