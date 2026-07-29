//! Shared small primitives used by every popup kind

use gtk::pango::{EllipsizeMode, WrapMode};
use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{hooks, NotificationView};
use unixnotis_ui::presentation::build_semantic_badge;

use super::super::commands::try_send_command;
use super::super::presentation::{PopupEntryViewModel, PopupTrustPresentation, ReplyPresentation};
use crate::dbus::UiCommand;
use crate::ui::UiState;

pub(super) struct IdentityAvatar {
    pub(super) widget: gtk::Box,
}

pub(super) fn build_identity_avatar(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    size: i32,
) -> Option<IdentityAvatar> {
    let icon_size = (size - 14).max(18);
    let icon = build_semantic_badge(view.badge, icon_size)
        .or_else(|| state.build_app_icon_widget(notification, icon_size))?;
    icon.set_valign(Align::Center);
    icon.set_halign(Align::Center);
    icon.add_css_class("unixnotis-popup-icon");

    let avatar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    avatar.set_size_request(size, size);
    avatar.set_halign(Align::Start);
    avatar.set_valign(Align::Start);
    avatar.add_css_class("unixnotis-identity-avatar");
    avatar.add_css_class(view.trust.level.css_class());
    avatar.append(&icon);
    Some(IdentityAvatar { widget: avatar })
}

pub(super) fn build_identity_header(view: &PopupEntryViewModel) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.add_css_class("unixnotis-popup-header-row");
    header.set_margin_end(30);

    let app = gtk::Label::new(Some(&view.app_label));
    app.set_xalign(0.0);
    app.set_single_line_mode(true);
    app.set_ellipsize(EllipsizeMode::End);
    app.add_css_class("unixnotis-popup-app-name");
    if let Some(details) = view.trust.details_label.as_deref() {
        // Raw paths remain available on demand without entering normal card content
        app.set_tooltip_text(Some(details));
    }
    header.append(&app);

    if let Some(chip) = build_trust_chip(&view.trust) {
        header.append(&chip);
    }

    header.append(&build_header_spacer());
    header.append(&build_urgency_badge(view.critical));

    let time = gtk::Label::new(Some(&view.timestamp_label));
    time.set_single_line_mode(true);
    time.add_css_class("unixnotis-popup-time");
    header.append(&time);
    header
}

pub(super) fn build_title_label(view: &PopupEntryViewModel) -> Option<gtk::Label> {
    if view.title.trim().is_empty() {
        return None;
    }

    let title = gtk::Label::new(Some(&view.title));
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_wrap_mode(WrapMode::WordChar);
    title.set_ellipsize(EllipsizeMode::End);
    title.set_lines(2);
    title.add_css_class("unixnotis-popup-summary");
    Some(title)
}

pub(super) fn build_body_label(view: &PopupEntryViewModel, line_limit: i32) -> Option<gtk::Label> {
    let body_text = view.body.as_deref()?;
    let body = gtk::Label::new(Some(body_text));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_wrap_mode(WrapMode::WordChar);
    body.set_ellipsize(EllipsizeMode::End);
    body.set_lines(line_limit);
    body.add_css_class("unixnotis-popup-body");
    Some(body)
}

pub(super) fn build_reply_note(view: &PopupEntryViewModel) -> Option<gtk::Label> {
    if view.trust.reply != ReplyPresentation::Unavailable {
        return None;
    }

    let note = gtk::Label::new(Some("Reply unavailable"));
    note.set_xalign(0.0);
    note.add_css_class("unixnotis-popup-footer-note");
    Some(note)
}

pub(super) fn build_secondary_claim(view: &PopupEntryViewModel) -> Option<gtk::Label> {
    let text = view.secondary_claim.as_deref()?;
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(WrapMode::WordChar);
    label.add_css_class("unixnotis-popup-secondary-claim");
    Some(label)
}

pub(in crate::ui::entry) fn build_close_button() -> gtk::Button {
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("unixnotis-popup-close");
    close.set_halign(Align::End);
    close.set_tooltip_text(Some("Dismiss notification"));
    close
}

pub(in crate::ui::entry) fn build_action_row(
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    notification: unixnotis_core::NotificationKey,
    view: &PopupEntryViewModel,
) -> Option<gtk::Box> {
    if view.primary_actions.is_empty() && view.overflow_actions.is_empty() {
        return None;
    }

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class("unixnotis-popup-actions");
    for action in &view.primary_actions {
        actions.append(&build_action_button(command_tx, notification, action, None));
    }
    if !view.overflow_actions.is_empty() {
        actions.append(&build_overflow_menu(command_tx, notification, view));
    }
    Some(actions)
}

fn build_action_button(
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    notification: unixnotis_core::NotificationKey,
    action: &super::super::presentation::ActionViewModel,
    popover: Option<&gtk::Popover>,
) -> gtk::Button {
    let button = gtk::Button::with_label(&action.label);
    button.add_css_class("unixnotis-popup-action");
    let action_key = action.key.clone();
    let tx = command_tx.clone();
    let popover = popover.cloned();
    button.connect_clicked(move |_| {
        // Menus close before the exact daemon-validated action is queued
        if let Some(popover) = &popover {
            popover.popdown();
        }
        try_send_command(
            &tx,
            UiCommand::InvokeAction {
                notification,
                action_key: action_key.clone(),
            },
        );
    });
    button
}

fn build_overflow_menu(
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    notification: unixnotis_core::NotificationKey,
    view: &PopupEntryViewModel,
) -> gtk::MenuButton {
    let menu = gtk::MenuButton::new();
    menu.set_icon_name("view-more-symbolic");
    menu.set_tooltip_text(Some("More actions"));
    menu.add_css_class("unixnotis-popup-action-overflow");

    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.add_css_class("unixnotis-popup-action-overflow-list");
    for action in &view.overflow_actions {
        list.append(&build_action_button(
            command_tx,
            notification,
            action,
            Some(&popover),
        ));
    }
    popover.set_child(Some(&list));
    menu.set_popover(Some(&popover));
    menu
}

fn build_urgency_badge(is_critical: bool) -> gtk::Label {
    let badge = gtk::Label::new(Some("Critical"));
    // The stable node keeps header spacing predictable across urgency changes
    badge.add_css_class(hooks::urgency::BADGE);
    badge.set_single_line_mode(true);
    badge.set_visible(is_critical);
    badge
}

fn build_trust_chip(trust: &PopupTrustPresentation) -> Option<gtk::Label> {
    let label = trust.short_label.as_deref()?;
    let chip = gtk::Label::new(Some(label));
    chip.set_single_line_mode(true);
    chip.add_css_class("unixnotis-popup-trust-chip");
    chip.add_css_class(trust.level.css_class());
    if let Some(details) = trust.details_label.as_deref() {
        // Detailed evidence remains one hover or keyboard query away
        chip.set_tooltip_text(Some(details));
    }
    Some(chip)
}

fn build_header_spacer() -> gtk::Box {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    // The expanding spacer anchors time and close controls to the trailing edge
    spacer.set_hexpand(true);
    spacer
}

#[cfg(test)]
#[path = "tests/common.rs"]
mod tests;
