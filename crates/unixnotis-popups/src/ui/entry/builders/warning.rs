//! Restrained warning popup for conflicting application identity

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::NotificationView;
use unixnotis_ui::presentation::build_semantic_badge;

use super::common::{
    build_body_label, build_identity_header, build_reply_note, build_secondary_claim,
    build_title_label,
};
use super::{append_thumbnail, RenderedPopup};
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

const WARNING_ICON_SIZE: i32 = 20;

pub(super) fn build_warning_popup(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    close: &gtk::Button,
) -> RenderedPopup {
    let main = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    main.add_css_class("unixnotis-popup-warning-content");

    let icon = build_semantic_badge(view.badge, WARNING_ICON_SIZE)
        .or_else(|| state.build_app_icon_widget(notification, WARNING_ICON_SIZE));
    let has_icon = if let Some(icon) = icon {
        // Conflict attribution supplies a daemon-owned generic badge instead of claimed branding
        icon.set_halign(Align::Start);
        icon.set_valign(Align::Start);
        icon.add_css_class("unixnotis-popup-icon");
        icon.add_css_class("unixnotis-popup-warning-icon");
        main.append(&icon);
        true
    } else {
        false
    };

    let content = gtk::Box::new(gtk::Orientation::Vertical, 3);
    content.set_hexpand(true);
    let header = build_identity_header(state, notification, view, close, None);
    content.append(&header.widget);
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
        has_icon,
        has_image,
    }
}
