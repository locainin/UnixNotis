//! Notification action button rebuilding and dispatch

use std::borrow::Cow;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::NotificationView;

use crate::control::UiCommand;
use crate::ui::panel::input::ClickCooldown;
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
    configure_inline_reply(&row.inline_reply, notification, is_active);
    // Fast path skips button rebuilding when the action set is unchanged
    {
        let cached = row.action_cache.borrow();
        let reply_cached = row.reply_cache.borrow();
        if row.action_cache_id.get() == notification.id
            && cached.len() == notification.actions.len()
            && cached
                .iter()
                .zip(notification.actions.iter())
                .all(|((key, label), action)| key == &action.key && label == &action.label)
            && reply_cached.0 == notification.inline_reply
            && reply_cached.1 == is_active
        {
            return;
        }
    }

    {
        // Cache the current action signature for the next update cycle
        let mut cached = row.action_cache.borrow_mut();
        cached.clear();
        cached.reserve(notification.actions.len());
        for action in &notification.actions {
            cached.push((action.key.clone(), action.label.clone()));
        }
        row.action_cache_id.set(notification.id);
        *row.reply_cache.borrow_mut() = (notification.inline_reply.clone(), is_active);
    }

    // Old buttons leave before rebuilding the current action set
    while let Some(child) = row.actions_box.first_child() {
        row.actions_box.remove(&child);
    }
    if visible_action_count(notification, is_active) == 0 {
        return;
    }

    let mut reply_button_added = false;
    for action in &notification.actions {
        if action.key == "inline-reply" {
            if reply_button_added || !is_active || !notification.inline_reply.available {
                continue;
            }
            reply_button_added = true;
            let label = if !notification.inline_reply.label.is_empty() {
                notification.inline_reply.label.as_str()
            } else if !action.label.is_empty() {
                action.label.as_str()
            } else {
                "Reply"
            };
            let button = gtk::Button::with_label(clamp_action_label_text(label).as_ref());
            button.add_css_class("unixnotis-panel-action");
            button.add_css_class("unixnotis-notification-action");
            connect_inline_reply_button(&button, &row.inline_reply);
            row.actions_box.append(&button);
            continue;
        }

        // Bound action text before GTK measures the button
        let button = gtk::Button::with_label(clamp_action_label_text(&action.label).as_ref());
        button.add_css_class("unixnotis-panel-action");
        button.add_css_class("unixnotis-notification-action");
        let action_key = action.key.clone();
        let tx = command_tx.clone();
        let id = notification.id;
        let action_gate = ClickCooldown::new(Duration::from_millis(ACTION_BUTTON_GUARD_MS));
        button.connect_clicked(move |_| {
            if !action_gate.try_start() {
                return;
            }
            debug!(id, action = %action_key, "action invoked");
            // The closure keeps its own key copy so the button can outlive the loop frame
            try_send_command(
                &tx,
                UiCommand::InvokeAction {
                    id,
                    action_key: action_key.clone(),
                },
            );
        });
        row.actions_box.append(&button);
    }
}

pub(super) fn visible_action_count(notification: &NotificationView, is_active: bool) -> usize {
    let regular = notification
        .actions
        .iter()
        .filter(|action| action.key != "inline-reply")
        .count();
    let reply = is_active
        && notification.inline_reply.available
        && notification
            .actions
            .iter()
            .any(|action| action.key == "inline-reply");
    regular + usize::from(reply)
}
