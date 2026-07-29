//! Communication popup with quiet application identity and message-first hierarchy

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::common::{
    build_body_label, build_identity_avatar, build_identity_header, build_reply_note,
    build_secondary_claim, build_title_label,
};
use super::{append_thumbnail, RenderedPopup};
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

const COMMUNICATION_AVATAR_SIZE: i32 = 44;

pub(super) fn build_communication_popup(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
) -> RenderedPopup {
    let main = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    main.add_css_class("unixnotis-popup-communication-content");
    let avatar = build_identity_avatar(state, notification, view, COMMUNICATION_AVATAR_SIZE);
    if let Some(avatar) = avatar.as_ref() {
        main.append(&avatar.widget);
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 3);
    content.set_hexpand(true);

    // Communication cards read as app identity, sender, then message preview
    content.append(&build_identity_header(view));
    if let Some(claim) = build_secondary_claim(view) {
        content.append(&claim);
    }
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
    main.append(&content);

    RenderedPopup {
        widget: main,
        has_icon: avatar.is_some(),
        has_image,
    }
}
