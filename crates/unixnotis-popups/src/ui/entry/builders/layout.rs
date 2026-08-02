//! Shared three-column popup composition

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::common::{
    build_body_label, build_identity_avatar, build_identity_header, build_reply_note,
    build_secondary_claim, build_title_label,
};
use super::{append_thumbnail, RenderedPopup};
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

const POPUP_IDENTITY_SIZE: i32 = 34;

pub(super) struct PopupLayout {
    pub(super) css_class: &'static str,
    pub(super) body_lines: i32,
    pub(super) show_reply_note: bool,
}

pub(super) fn build_popup_grid(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    layout: PopupLayout,
) -> RenderedPopup {
    let grid = gtk::Grid::new();
    grid.add_css_class(layout.css_class);
    grid.add_css_class("unixnotis-popup-content-grid");
    grid.set_column_spacing(8);
    grid.set_row_spacing(4);
    grid.set_hexpand(true);
    grid.set_accessible_role(gtk::AccessibleRole::Group);
    let accessible_label = popup_accessible_label(view);
    grid.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);

    let avatar = build_identity_avatar(state, notification, view, POPUP_IDENTITY_SIZE);
    grid.attach(&avatar.widget, 0, 0, 1, 2);

    let header = build_identity_header(view);
    grid.attach(&header.identity, 1, 0, 1, 1);
    grid.attach(&header.trailing, 2, 0, 1, 1);

    let message = gtk::Box::new(gtk::Orientation::Vertical, 2);
    message.add_css_class("unixnotis-popup-message");
    message.set_hexpand(true);
    if let Some(claim) = build_secondary_claim(view) {
        message.append(&claim);
    }
    if let Some(title) = build_title_label(view) {
        message.append(&title);
    }
    if let Some(body) = build_body_label(view, layout.body_lines) {
        message.append(&body);
    }
    let has_image = append_thumbnail(notification, view, &message);
    if layout.show_reply_note {
        if let Some(note) = build_reply_note(view) {
            message.append(&note);
        }
    }
    grid.attach(&message, 1, 1, 2, 1);

    RenderedPopup {
        widget: grid,
        has_icon: true,
        has_image,
    }
}

fn popup_accessible_label(view: &PopupEntryViewModel) -> String {
    let mut parts = vec![view.app_label.trim()];
    if let Some(trust) = view.trust.short_label.as_deref() {
        parts.push(trust.trim());
    }
    if let Some(claim) = view.secondary_claim.as_deref() {
        parts.push(claim.trim());
    }
    if !view.title.trim().is_empty() {
        parts.push(view.title.trim());
    }
    if let Some(body) = view.body.as_deref().filter(|body| !body.trim().is_empty()) {
        parts.push(body.trim());
    }
    parts.join(". ")
}

#[cfg(test)]
#[path = "tests/layout.rs"]
mod tests;
