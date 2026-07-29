//! Compact utility popup for device, transfer, clipboard, and generic events

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::common::{
    build_body_label, build_identity_avatar, build_identity_header, build_secondary_claim,
    build_title_label,
};
use super::{append_thumbnail, RenderedPopup};
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

const UTILITY_AVATAR_SIZE: i32 = 36;

pub(super) fn build_utility_popup(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
) -> RenderedPopup {
    let main = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    main.add_css_class("unixnotis-popup-utility-content");

    let avatar = build_identity_avatar(state, notification, view, UTILITY_AVATAR_SIZE);
    if let Some(avatar) = avatar.as_ref() {
        main.append(&avatar.widget);
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
    content.set_hexpand(true);
    content.append(&build_identity_header(view));
    if let Some(claim) = build_secondary_claim(view) {
        content.append(&claim);
    }
    if let Some(title) = build_title_label(view) {
        content.append(&title);
    }
    if let Some(body) = build_body_label(view, 2) {
        content.append(&body);
    }
    let has_image = append_thumbnail(state, notification, view, &content);
    main.append(&content);

    RenderedPopup {
        widget: main,
        has_icon: avatar.is_some(),
        has_image,
    }
}
