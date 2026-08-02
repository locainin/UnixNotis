//! Shared small primitives used by every popup kind

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gtk::glib;
use gtk::pango::{EllipsizeMode, WrapMode};
use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{hooks, NotificationView};
use unixnotis_ui::presentation::{action_activation, build_semantic_badge, ActionActivation};

use super::super::commands::try_send_command;
use super::super::presentation::{
    PopupEntryViewModel, PopupKind, PopupTrustPresentation, ReplyPresentation,
};
use crate::dbus::UiCommand;
use crate::ui::entry::activation::mark_interactive;
use crate::ui::UiState;

// Clicks inside this window after arming are treated as accidental double-taps
const MIN_CONFIRM_INTERVAL_MS: u64 = 350;
// Armed state expires after this long and the button goes back to normal
const MAX_CONFIRM_TIMEOUT_MS: u64 = 5000;

pub(super) struct IdentityAvatar {
    pub(super) widget: gtk::Box,
}

pub(super) fn build_identity_avatar(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    size: i32,
) -> IdentityAvatar {
    let icon_size = (size - 14).max(18);
    let icon = if view.kind == PopupKind::Communication {
        UiState::build_conversation_avatar_widget(notification, icon_size)
    } else {
        None
    }
    .or_else(|| build_semantic_badge(view.badge, icon_size))
    .or_else(|| state.build_app_icon_widget(notification, icon_size))
    .unwrap_or_else(|| gtk::Image::from_icon_name("application-x-executable-symbolic"));
    icon.set_pixel_size(icon_size);
    icon.set_size_request(icon_size, icon_size);
    icon.set_valign(Align::Center);
    icon.set_halign(Align::Center);
    // Expansion centers the glyph optically inside the fixed avatar allocation
    icon.set_hexpand(true);
    icon.set_vexpand(true);
    icon.set_accessible_role(gtk::AccessibleRole::Presentation);
    icon.add_css_class("unixnotis-popup-icon");

    let avatar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    avatar.set_size_request(size, size);
    avatar.set_halign(Align::Start);
    avatar.set_valign(Align::Start);
    avatar.add_css_class("unixnotis-identity-avatar");
    avatar.add_css_class(view.trust.level.css_class());
    avatar.append(&icon);
    IdentityAvatar { widget: avatar }
}

pub(super) struct IdentityHeader {
    pub(super) identity: gtk::Box,
    pub(super) trailing: gtk::Box,
}

pub(super) fn build_identity_header(view: &PopupEntryViewModel) -> IdentityHeader {
    let identity = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    identity.add_css_class("unixnotis-popup-identity-row");
    identity.set_hexpand(true);
    identity.set_halign(Align::Fill);

    let app = gtk::Label::new(Some(&view.app_label));
    app.set_xalign(0.0);
    app.set_hexpand(true);
    app.set_halign(Align::Fill);
    app.set_single_line_mode(true);
    app.set_ellipsize(EllipsizeMode::End);
    app.add_css_class("unixnotis-popup-app-name");
    if let Some(details) = view.trust.details_label.as_deref() {
        // Raw paths remain available on demand without entering normal card content
        app.set_tooltip_text(Some(details));
    }
    identity.append(&app);

    if let Some(chip) = build_trust_chip(&view.trust) {
        identity.append(&chip);
    }

    let trailing = gtk::Box::new(gtk::Orientation::Vertical, 2);
    trailing.add_css_class("unixnotis-popup-trailing");
    trailing.set_halign(Align::End);
    trailing.set_valign(Align::Start);
    trailing.set_margin_end(30);

    let time = gtk::Label::new(Some(&view.timestamp_label));
    time.set_single_line_mode(true);
    time.set_halign(Align::End);
    time.add_css_class("unixnotis-popup-time");
    trailing.append(&time);

    let urgency = build_urgency_badge(view.critical);
    urgency.set_halign(Align::End);
    trailing.append(&urgency);
    IdentityHeader { identity, trailing }
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
    // Provenance context stays one quiet metadata line instead of changing card height
    label.set_single_line_mode(true);
    label.set_ellipsize(EllipsizeMode::End);
    label.add_css_class("unixnotis-popup-secondary-claim");
    Some(label)
}

pub(in crate::ui::entry) fn build_close_button() -> gtk::Button {
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("unixnotis-popup-close");
    close.set_halign(Align::End);
    close.set_tooltip_text(Some("Dismiss notification"));
    mark_interactive(&close);
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
    mark_interactive(&button);
    let action_key = action.key.clone();
    let original_label = action.label.clone();
    let policy = action.policy;
    let tx = command_tx.clone();
    let popover = popover.cloned();
    // Single shared state: None = not armed, Some(instant) = armed at that time
    // Using Rc<Cell<>> so both the click handler and the timeout callback read and
    // write the same cell. The timeout captures `now` at arm time and only resets
    // the button if that exact timestamp is still current — this prevents a stale
    // timer from the first cycle from wiping the visual state of a newer cycle.
    let armed_at = Rc::new(Cell::new(None::<Instant>));
    button.connect_clicked(move |button| {
        let confirmed = match action_activation(policy, armed_at.get().is_some()) {
            ActionActivation::Denied => return,
            ActionActivation::ArmConfirmation => {
                let now = Instant::now();
                armed_at.set(Some(now));
                let confirmation_label = format!("Confirm {original_label}");
                button.set_label(&confirmation_label);
                button.set_tooltip_text(Some("Activate again to confirm"));
                button.update_property(&[gtk::accessible::Property::Label(&confirmation_label)]);
                // Clean up the armed state after a timeout so the button does not stay in
                // confirm mode forever
                let expire_button = button.clone();
                let expire_label = original_label.clone();
                let expire_armed_at = Rc::clone(&armed_at);
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(MAX_CONFIRM_TIMEOUT_MS),
                    move || {
                        if expire_armed_at.get() == Some(now) {
                            expire_armed_at.set(None);
                            expire_button.set_label(&expire_label);
                            expire_button.set_tooltip_text(None);
                            expire_button.update_property(&[gtk::accessible::Property::Label(
                                &expire_label,
                            )]);
                        }
                    },
                );
                return;
            }
            ActionActivation::Invoke { confirmed } => {
                // Only check timing when the action was actually confirmed
                // Allow-policy actions skip this path entirely
                if confirmed {
                    let elapsed = armed_at.get().map(|t| t.elapsed());
                    match elapsed {
                        // No arm time recorded means something went wrong
                        // Clean up instead of dispatching
                        None => {
                            armed_at.set(None);
                            button.set_label(&original_label);
                            button.set_tooltip_text(None);
                            button.update_property(&[gtk::accessible::Property::Label(
                                &original_label,
                            )]);
                            return;
                        }
                        // Click came too fast after arming
                        // Probably an accidental double-tap, stay armed so the next click
                        // can still go through
                        Some(d)
                            if d < std::time::Duration::from_millis(MIN_CONFIRM_INTERVAL_MS) =>
                        {
                            return;
                        }
                        // Confirmation took too long
                        // Reset the button and make the person re-arm
                        Some(d) if d > std::time::Duration::from_millis(MAX_CONFIRM_TIMEOUT_MS) => {
                            armed_at.set(None);
                            button.set_label(&original_label);
                            button.set_tooltip_text(None);
                            button.update_property(&[gtk::accessible::Property::Label(
                                &original_label,
                            )]);
                            return;
                        }
                        // Right amount of time passed, dispatch the action
                        _ => {}
                    }
                }
                confirmed
            }
        };
        // Reset everything after a successful dispatch
        // The next click will start a fresh confirmation cycle instead of invoking again
        armed_at.set(None);
        button.set_label(&original_label);
        button.set_tooltip_text(None);
        button.update_property(&[gtk::accessible::Property::Label(&original_label)]);
        // Menus close only after an action passes its confirmation policy
        if let Some(popover) = &popover {
            popover.popdown();
        }
        try_send_command(
            &tx,
            UiCommand::InvokeAction {
                notification,
                action_key: action_key.clone(),
                confirmed,
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
    mark_interactive(&menu);

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
    let badge = gtk::Label::new(Some("!"));
    // The stable node keeps header spacing predictable across urgency changes
    badge.add_css_class(hooks::urgency::BADGE);
    badge.set_single_line_mode(true);
    badge.set_tooltip_text(Some("Critical notification"));
    badge.update_property(&[gtk::accessible::Property::Label("Critical notification")]);
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

#[cfg(test)]
#[path = "tests/common.rs"]
mod tests;
