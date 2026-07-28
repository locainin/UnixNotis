//! Communication popup with quiet application identity and message-first hierarchy

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::common::{build_body_label, build_identity_header, build_reply_note, build_title_label};
use super::{append_thumbnail, RenderedPopup};
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

const COMMUNICATION_APP_ICON_SIZE: i32 = 20;

pub(super) fn build_communication_popup(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    close: &gtk::Button,
) -> RenderedPopup {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 3);
    content.add_css_class("unixnotis-popup-communication-content");

    // Communication cards read as app identity, sender, then message preview
    let header = build_identity_header(
        state,
        notification,
        view,
        close,
        Some(COMMUNICATION_APP_ICON_SIZE),
    );
    content.append(&header.widget);
    if let Some(title) = build_title_label(view) {
        content.append(&title);
    }
    if let Some(body) = build_body_label(view, 3) {
        content.append(&body);
    }
    let has_image = append_thumbnail(state, notification, view, &content);
    if let Some(note) = build_reply_note(view) {
        content.append(&note);
    }

    RenderedPopup {
        widget: content,
        has_icon: header.has_icon,
        has_image,
    }
}
