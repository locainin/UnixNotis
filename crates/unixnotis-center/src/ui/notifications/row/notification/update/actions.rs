//! Notification action button rebuilding and dispatch

use std::borrow::Cow;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::NotificationView;
use unixnotis_ui::presentation::{
    action_activation, ActionActivation, NotificationPresentation, ReplyPresentation,
};

use crate::control::UiCommand;
use crate::ui::panel::behavior::input::ClickCooldown;
use crate::ui::try_send_command;

use super::super::reply::{configure_inline_reply, connect_inline_reply_button};
use super::super::state::{NotificationRowWidgets, MAX_ACTION_LABEL_CHARS};
use super::labels::clamp_label_text;

const ACTION_BUTTON_GUARD_MS: u64 = 180;

pub(super) fn clamp_action_label_text(text: &str) -> Cow<'_, str> {
    // Action text uses the same clamp rule every time so row width stays stable
    // This keeps the panel from being stretched by one bad button label
    clamp_label_text(text, MAX_ACTION_LABEL_CHARS)
}

pub(super) fn update_actions(
    row: &NotificationRowWidgets,
    command_tx: &mpsc::Sender<UiCommand>,
    notification: &Rc<NotificationView>,
    is_active: bool,
) {
    let presentation = NotificationPresentation::from_view(notification);
    configure_inline_reply(&row.inline_reply, notification, is_active);
    let action_signature = action_signature(&presentation, is_active);
    // Fast path skips button rebuilding when the action set is unchanged
    {
        let cached = row.action_cache.borrow();
        let reply_cached = row.reply_cache.borrow();
        if row.action_cache_key.get() == notification.key()
            && cached.as_slice() == action_signature.as_slice()
            && reply_cached.0 == notification.inline_reply
            && reply_cached.1 == notification.inline_reply_policy
            && reply_cached.2 == is_active
        {
            return;
        }
    }

    {
        // Cache the current action signature for the next update cycle
        let mut cached = row.action_cache.borrow_mut();
        cached.clear();
        cached.extend(action_signature);
        row.action_cache_key.set(notification.key());
        *row.reply_cache.borrow_mut() = (
            notification.inline_reply.clone(),
            notification.inline_reply_policy,
            is_active,
        );
    }

    // Old buttons leave before rebuilding the current action set
    while let Some(child) = row.actions_box.first_child() {
        row.actions_box.remove(&child);
    }
    // Archived notifications cannot be valid daemon action targets
    if !is_active {
        return;
    }
    if visible_action_count_from(&presentation, is_active) == 0 {
        return;
    }

    if is_active && presentation.trust.reply == ReplyPresentation::Available {
        let action_label = notification
            .actions
            .iter()
            .find(|action| action.key == "inline-reply")
            .map(|action| action.label.as_str())
            .unwrap_or_default();
        let label = if !notification.inline_reply.label.is_empty() {
            notification.inline_reply.label.as_str()
        } else if !action_label.is_empty() {
            action_label
        } else {
            "Reply"
        };
        let button = gtk::Button::with_label(clamp_action_label_text(label).as_ref());
        button.add_css_class("unixnotis-panel-action");
        button.add_css_class("unixnotis-notification-action");
        connect_inline_reply_button(&button, &row.inline_reply);
        row.actions_box.append(&button);
    }

    for action in &presentation.actions.primary {
        let button = build_action_button(command_tx, notification.key(), action);
        row.actions_box.append(&button);
    }
    if !presentation.actions.overflow.is_empty() {
        row.actions_box.append(&build_overflow_menu(
            command_tx,
            notification.key(),
            &presentation.actions.overflow,
        ));
    }
    if let Some(default_key) = blank_default_action_key(&presentation) {
        row.actions_box.append(&build_default_action_button(
            command_tx,
            notification.key(),
            default_key,
        ));
    }
}

fn action_signature(
    presentation: &NotificationPresentation,
    is_active: bool,
) -> Vec<(String, String, unixnotis_core::ApplicationActionPolicy)> {
    if !is_active {
        return Vec::new();
    }
    let mut signature = presentation
        .actions
        .primary
        .iter()
        .chain(&presentation.actions.overflow)
        .map(|action| (action.key.clone(), action.label.clone(), action.policy))
        .collect::<Vec<_>>();
    if let Some(default_key) = blank_default_action_key(presentation) {
        // The empty label distinguishes the compact icon-only default control
        signature.push((
            default_key.to_string(),
            String::new(),
            unixnotis_core::ApplicationActionPolicy::Allow,
        ));
    }
    signature
}

fn blank_default_action_key(presentation: &NotificationPresentation) -> Option<&str> {
    // Shared presentation keeps allowed defaults out of the visible button lists
    presentation.actions.default_key.as_deref()
}

fn build_default_action_button(
    command_tx: &mpsc::Sender<UiCommand>,
    notification: unixnotis_core::NotificationKey,
    action_key: &str,
) -> gtk::Button {
    let button = gtk::Button::from_icon_name("document-open-symbolic");
    button.add_css_class("unixnotis-panel-action");
    button.add_css_class("unixnotis-notification-action");
    button.add_css_class("unixnotis-panel-default-action");
    button.set_tooltip_text(Some("Open notification"));
    button.update_property(&[gtk::accessible::Property::Label("Open notification")]);
    let action_key = action_key.to_string();
    let tx = command_tx.clone();
    let action_gate = ClickCooldown::new(Duration::from_millis(ACTION_BUTTON_GUARD_MS));
    button.connect_clicked(move |_| {
        if !action_gate.try_start() {
            return;
        }
        try_send_command(
            &tx,
            UiCommand::InvokeAction {
                notification,
                action_key: action_key.clone(),
                confirmed: false,
            },
        );
    });
    button
}

fn build_action_button(
    command_tx: &mpsc::Sender<UiCommand>,
    notification: unixnotis_core::NotificationKey,
    action: &unixnotis_ui::presentation::ActionView,
) -> gtk::Button {
    // Bound action text before GTK measures the button
    let button = gtk::Button::with_label(clamp_action_label_text(&action.label).as_ref());
    button.add_css_class("unixnotis-panel-action");
    button.add_css_class("unixnotis-notification-action");
    let action_key = action.key.clone();
    let original_label = clamp_action_label_text(&action.label).into_owned();
    let policy = action.policy;
    let tx = command_tx.clone();
    let confirmation_armed = Cell::new(false);
    let action_gate = ClickCooldown::new(Duration::from_millis(ACTION_BUTTON_GUARD_MS));
    button.connect_clicked(move |button| {
        if !action_gate.try_start() {
            return;
        }
        let confirmed = match action_activation(policy, confirmation_armed.get()) {
            ActionActivation::Denied => return,
            ActionActivation::ArmConfirmation => {
                confirmation_armed.set(true);
                let confirmation_label = format!("Confirm {original_label}");
                button.set_label(&confirmation_label);
                button.set_tooltip_text(Some("Activate again to confirm"));
                button.update_property(&[gtk::accessible::Property::Label(&confirmation_label)]);
                return;
            }
            ActionActivation::Invoke { confirmed } => confirmed,
        };
        debug!(
            id = notification.id,
            generation = notification.generation,
            action = %action_key,
            "action invoked"
        );
        // The closure keeps its own key copy so the button can outlive the loop frame
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
    command_tx: &mpsc::Sender<UiCommand>,
    notification: unixnotis_core::NotificationKey,
    actions: &[unixnotis_ui::presentation::ActionView],
) -> gtk::MenuButton {
    let menu = gtk::MenuButton::new();
    menu.set_icon_name("view-more-symbolic");
    menu.set_tooltip_text(Some("More actions"));
    menu.add_css_class("unixnotis-panel-action-overflow");

    let popover = gtk::Popover::new();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.add_css_class("unixnotis-panel-action-overflow-list");
    for action in actions {
        list.append(&build_action_button(command_tx, notification, action));
    }
    popover.set_child(Some(&list));
    menu.set_popover(Some(&popover));
    menu
}

pub(super) fn visible_action_count(notification: &NotificationView, is_active: bool) -> usize {
    visible_action_count_from(
        &NotificationPresentation::from_view(notification),
        is_active,
    )
}

fn visible_action_count_from(presentation: &NotificationPresentation, is_active: bool) -> usize {
    if !is_active {
        return 0;
    }
    let regular = presentation.actions.primary.len() + presentation.actions.overflow.len();
    let reply = presentation.trust.reply == ReplyPresentation::Available;
    let blank_default = blank_default_action_key(presentation).is_some();
    regular + usize::from(reply) + usize::from(blank_default)
}
