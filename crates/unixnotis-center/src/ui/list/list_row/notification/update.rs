//! Notification row refresh logic
//!
//! This file owns the repeated update rules for reused notification rows

use std::borrow::Cow;
use std::cell::RefCell;
use std::time::Duration;

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::{hooks, NotificationView, Urgency};

use crate::dbus::UiCommand;
use crate::ui::icons::IconResolver;
use crate::ui::input_guard::ClickCooldown;
use crate::ui::try_send_command;

use super::super::super::list_item::RowData;
use super::state::{
    IconSignature, NotificationRowWidgets, OptionalLabelState, MAX_ACTION_LABEL_CHARS,
    MAX_BODY_LABEL_CHARS, MAX_SUMMARY_LABEL_CHARS,
};

const ACTION_BUTTON_GUARD_MS: u64 = 180;

pub(in crate::ui::list) fn update_notification_row(
    row: &NotificationRowWidgets,
    root: &gtk::Box,
    data: &RowData,
    icon_resolver: &IconResolver,
    command_tx: &mpsc::Sender<UiCommand>,
) {
    // Recycled rows can be updated with None while model changes
    // Nothing should touch the GTK children until the row has real data again
    let Some(notification) = data.notification.as_ref() else {
        return;
    };
    let notification = notification.as_ref();

    // CSS state toggles stay explicit so stale visual state cannot linger
    set_class_state(
        root,
        hooks::shared_state::CRITICAL,
        notification.urgency == Urgency::Critical as u8,
    );
    // Active rows can be styled differently from history rows
    set_class_state(root, hooks::shared_state::ACTIVE, data.is_active);
    // Stacked class indicates collapsed entries in grouped mode
    set_class_state(root, hooks::shared_state::STACKED, data.stacked);

    // Extra state classes give themes better hooks without changing old selectors
    set_class_state(
        root,
        hooks::panel_card::HAS_SUMMARY,
        has_visible_text(&notification.summary),
    );
    set_class_state(
        root,
        hooks::panel_card::HAS_BODY,
        has_visible_text(&notification.body),
    );
    set_class_state(
        root,
        hooks::panel_card::HAS_ACTIONS,
        !notification.actions.is_empty(),
    );
    set_class_state(
        root,
        hooks::panel_card::NO_ACTIONS,
        notification.actions.is_empty(),
    );
    // App name always renders, even when summary or body are missing
    set_label_text_if_changed(&row.app_label, &notification.app_name);
    // Clamp before GTK rendering to avoid giant layout passes
    update_summary_label(&row.summary_label, &notification.summary);
    update_body_label(&row.body_label, &notification.body);
    row.notify_id.set(notification.id);

    update_actions(
        &row.actions_box,
        &row.action_cache,
        command_tx,
        notification,
    );

    // Icon decode and apply is skipped when the icon signature is unchanged
    // Text and action changes should not trigger another icon pipeline round
    let next_sig = IconSignature::from(notification);
    let mut sig_guard = row.icon_sig.borrow_mut();
    if sig_guard.as_ref() != Some(&next_sig) {
        let scale = root.scale_factor();
        icon_resolver.apply_icon(&row.icon, notification, 22, scale);
        *sig_guard = Some(next_sig);
    }
}

pub(super) fn optional_label_state(text: &str, max_chars: usize) -> OptionalLabelState<'_> {
    if !has_visible_text(text) {
        // Empty text rows stay hidden so card spacing stays honest
        return OptionalLabelState {
            visible: false,
            text: Cow::Borrowed(""),
        };
    }
    if max_chars == 0 {
        // Zero-char clamps are an explicit request to collapse the row
        return OptionalLabelState {
            visible: false,
            text: Cow::Borrowed(""),
        };
    }
    OptionalLabelState {
        visible: true,
        // Notification text stays plain so layout cannot be changed by markup
        text: clamp_label_text(text, max_chars),
    }
}

pub(super) fn clamp_action_label_text(text: &str) -> Cow<'_, str> {
    // Action text uses the same clamp rule every time so row width stays stable
    // This keeps the panel from being stretched by one bad button label
    clamp_label_text(text, MAX_ACTION_LABEL_CHARS)
}

fn update_summary_label(label: &gtk::Label, summary: &str) {
    // Summary rows collapse fully when the sender leaves the title empty
    update_optional_label(label, summary, MAX_SUMMARY_LABEL_CHARS);
}

fn update_body_label(label: &gtk::Label, body: &str) {
    // Body rows follow the same empty-text rule as summary rows
    update_optional_label(label, body, MAX_BODY_LABEL_CHARS);
}

fn update_optional_label(label: &gtk::Label, text: &str, max_chars: usize) {
    // Build the shared row state first so summary and body stay in sync
    // This keeps both rows on the same hide-or-clamp rules
    let state = optional_label_state(text, max_chars);
    set_label_visible_if_changed(label, state.visible);
    set_label_text_if_changed(label, state.text.as_ref());
}

fn has_visible_text(text: &str) -> bool {
    // Layout only needs to know if the row has real visible content
    text.chars().any(|ch| !ch.is_whitespace())
}

fn set_class_state(root: &gtk::Box, class_name: &str, enabled: bool) {
    // Reused rows are updated often
    // Guard CSS churn so GTK does not reprocess classes that already match
    if enabled {
        if !root.has_css_class(class_name) {
            root.add_css_class(class_name);
        }
    } else if root.has_css_class(class_name) {
        root.remove_css_class(class_name);
    }
}

fn set_label_visible_if_changed(label: &gtk::Label, visible: bool) {
    // Reused rows often receive the same visibility decision on every pass
    // Skip the setter so hidden and shown states stay quiet when unchanged
    if label.get_visible() != visible {
        label.set_visible(visible);
    }
}

fn set_label_text_if_changed(label: &gtk::Label, text: &str) {
    // Summary and body updates can be replayed many times while the row is stable
    // Compare against the current label so GTK only sees real text changes
    if label.text().as_str() != text {
        label.set_text(text);
    }
}

fn clamp_label_text(text: &str, max_chars: usize) -> Cow<'_, str> {
    if max_chars == 0 {
        // A zero cap means the caller wants the row blanked on purpose
        return Cow::Borrowed("");
    }
    // Iterate by character boundaries so UTF-8 stays valid after truncation
    for (chars, (idx, _)) in text.char_indices().enumerate() {
        if chars == max_chars {
            // Allocate only when truncation actually happens
            let mut clamped = String::with_capacity(idx + 3);
            clamped.push_str(&text[..idx]);
            clamped.push('…');
            return Cow::Owned(clamped);
        }
    }
    Cow::Borrowed(text)
}

fn update_actions(
    actions_box: &gtk::Box,
    cache: &RefCell<Vec<(String, String)>>,
    command_tx: &mpsc::Sender<UiCommand>,
    notification: &NotificationView,
) {
    // Fast path: skip button rebuild when the action set is unchanged
    // This avoids tearing down buttons during no-op refresh passes
    {
        let cached = cache.borrow();
        if cached.len() == notification.actions.len()
            && cached
                .iter()
                .zip(notification.actions.iter())
                .all(|((key, label), action)| key == &action.key && label == &action.label)
        {
            return;
        }
    }

    {
        // Cache the current action signature for the next update cycle
        // Reserve once so the cache grows with the current action count
        let mut cached = cache.borrow_mut();
        cached.clear();
        cached.reserve(notification.actions.len());
        for action in &notification.actions {
            cached.push((action.key.clone(), action.label.clone()));
        }
    }

    // Refresh action buttons only when the action list changes
    while let Some(child) = actions_box.first_child() {
        // Remove old buttons before rebuilding the new set
        actions_box.remove(&child);
    }
    if notification.actions.is_empty() {
        // No buttons should remain when the sender drops all actions
        return;
    }

    for action in &notification.actions {
        // Bound action text so one long label cannot stretch the whole row
        // Clamp before button creation so GTK never measures the oversized string
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
            // Action execution is best-effort and non-blocking
            // Best-effort enqueue keeps action handling responsive
            // The closure keeps its own key copy so the button can outlive the loop frame
            try_send_command(
                &tx,
                UiCommand::InvokeAction {
                    id,
                    action_key: action_key.clone(),
                },
            );
        });
        actions_box.append(&button);
    }
}
