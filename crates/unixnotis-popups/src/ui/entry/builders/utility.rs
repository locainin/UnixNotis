//! Compact utility popup for device, transfer, clipboard, and generic events

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::NotificationView;
use unixnotis_ui::presentation::build_semantic_badge;

use super::common::{build_body_label, build_identity_header, build_title_label};
use super::{append_thumbnail, RenderedPopup};
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

const UTILITY_ICON_SIZE: i32 = 24;

pub(super) fn build_utility_popup(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    close: &gtk::Button,
) -> RenderedPopup {
    let main = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    main.add_css_class("unixnotis-popup-utility-content");

    let icon = build_semantic_badge(view.badge, UTILITY_ICON_SIZE)
        .or_else(|| state.build_app_icon_widget(notification, UTILITY_ICON_SIZE));
    let has_icon = if let Some(icon) = icon {
        // Utility symbols support scanning without becoming the card's dominant object
        icon.set_halign(Align::Start);
        icon.set_valign(Align::Start);
        icon.add_css_class("unixnotis-popup-icon");
        icon.add_css_class("unixnotis-popup-utility-icon");
        main.append(&icon);
        true
    } else {
        false
    };

    let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
    content.set_hexpand(true);
    let header = build_identity_header(state, notification, view, close, None);
    content.append(&header.widget);
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
        has_icon,
        has_image,
    }
}
