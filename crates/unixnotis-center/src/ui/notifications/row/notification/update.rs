//! Notification row refresh logic
//!
//! This file owns the repeated update rules for reused notification rows

use std::borrow::Cow;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::{hooks, NotificationView, Urgency};

use crate::control::UiCommand;
use crate::ui::icons::IconResolver;
use crate::ui::panel::input::ClickCooldown;
use crate::ui::try_send_command;

use super::super::super::item::RowData;
use super::reply::{configure_inline_reply, connect_inline_reply_button};
use super::state::{
    IconSignature, NotificationRowWidgets, OptionalLabelState, MAX_ACTION_LABEL_CHARS,
    MAX_BODY_LABEL_CHARS, MAX_SUMMARY_LABEL_CHARS,
};

const ACTION_BUTTON_GUARD_MS: u64 = 180;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StackGhostVisibility {
    pub(super) middle: bool,
    pub(super) back: bool,
}

pub(super) const fn stack_ghost_visibility(stack_depth: u8) -> StackGhostVisibility {
    // A single rear layer uses the back slot because that slot starts without overlap
    // The middle slot becomes safe only when the back layer is present beneath it
    StackGhostVisibility {
        middle: stack_depth >= 2,
        back: stack_depth >= 1,
    }
}

pub(in crate::ui::notifications) fn update_notification_row(
    row: &NotificationRowWidgets,
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
    let card = &row.card;

    // State classes belong on the card, not the outer ListView row wrapper
    // CSS state toggles stay explicit so stale visual state cannot linger
    set_class_state(
        card,
        hooks::shared_state::CRITICAL,
        notification.urgency == Urgency::Critical as u8,
    );
    // Active rows can be styled differently from history rows
    set_class_state(card, hooks::shared_state::ACTIVE, data.is_active);
    // Stacked class indicates collapsed entries in grouped mode
    set_class_state(card, hooks::shared_state::STACKED, data.stacked);
    // Grouped cards are separate ListView rows, so direct hooks replace dead descendant CSS
    set_class_state(card, hooks::panel_card::GROUPED, true);
    // Collapsed and expanded hooks let themes space grouped cards directly
    set_class_state(card, hooks::panel_card::GROUP_COLLAPSED, data.stacked);
    set_class_state(card, hooks::panel_card::GROUP_EXPANDED, data.expanded);
    // Stack ghosts occupy fixed paint slots with different overlap rules
    // Depth one must skip the middle slot or its negative margin escapes the row
    let ghost_visibility = stack_ghost_visibility(data.stack_depth);
    set_widget_visible_if_changed(&row.stack_ghost_1, ghost_visibility.middle);
    set_widget_visible_if_changed(&row.stack_ghost_2, ghost_visibility.back);

    // Extra state classes give themes better hooks without changing old selectors
    set_class_state(
        card,
        hooks::panel_card::HAS_SUMMARY,
        has_visible_text(&notification.summary),
    );
    set_class_state(
        card,
        hooks::panel_card::HAS_BODY,
        has_visible_text(&notification.body),
    );
    let has_actions = visible_action_count(notification, data.is_active) > 0;
    set_class_state(card, hooks::panel_card::HAS_ACTIONS, has_actions);
    set_class_state(card, hooks::panel_card::NO_ACTIONS, !has_actions);
    let has_thumbnail =
        data.presentation.show_thumbnail && notification_has_thumbnail(notification);
    set_class_state(card, hooks::panel_card::HAS_THUMBNAIL, has_thumbnail);
    set_class_state(card, hooks::panel_card::NO_THUMBNAIL, !has_thumbnail);
    // App name always renders, even when summary or body are missing
    set_label_text_if_changed(&row.app_label, &notification.app_name);
    update_metadata_labels(row, data, notification);
    // Clamp before GTK rendering to avoid giant layout passes
    update_summary_label(&row.summary_label, &notification.summary);
    update_body_label(&row.body_label, &notification.body);
    row.notify_id.set(notification.id);

    update_actions(row, command_tx, notification, data.is_active);

    // Icon decode and apply is skipped when the icon signature is unchanged
    // Text and action changes should not trigger another icon pipeline round
    let next_sig = IconSignature::from(notification);
    let mut sig_guard = row.icon_sig.borrow_mut();
    let signature_changed = sig_guard.as_ref() != Some(&next_sig);
    if signature_changed {
        let scale = card.scale_factor();
        icon_resolver.apply_icon(&row.icon, notification, 22, scale);
        *sig_guard = Some(next_sig);
    }
    if has_thumbnail {
        // The icon cache handles repeat thumbnail lookups cheaply
        // Reapply while visible so config reloads cannot leave a stale preview
        let scale = card.scale_factor();
        icon_resolver.apply_icon(&row.thumbnail, notification, 56, scale);
    }
    set_widget_visible_if_changed(&row.thumbnail, has_thumbnail);
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

fn update_metadata_labels(
    row: &NotificationRowWidgets,
    data: &RowData,
    notification: &NotificationView,
) {
    set_widget_visible_if_changed(&row.meta_top, data.presentation.show_metadata);
    set_widget_visible_if_changed(&row.footer, data.presentation.show_metadata);
    if !data.presentation.show_metadata {
        // Disabled lanes collapse fully so default cards keep the older compact shape
        set_label_visible_if_changed(&row.meta_label, false);
        set_label_visible_if_changed(&row.time_badge, false);
        set_label_visible_if_changed(&row.footer_left, false);
        set_label_visible_if_changed(&row.footer_right, false);
        return;
    }

    let meta = notification_meta_label(notification);
    set_label_visible_if_changed(&row.meta_label, true);
    set_label_text_if_changed(&row.meta_label, &meta);

    let time_badge = relative_time_badge(data.presentation.received_at_ms);
    set_label_visible_if_changed(&row.time_badge, !time_badge.is_empty());
    set_label_text_if_changed(&row.time_badge, &time_badge);

    let footer_left = if notification.is_transient {
        "TRANSIENT"
    } else if data.is_active {
        "LIVE"
    } else {
        "HISTORY"
    };
    set_label_visible_if_changed(&row.footer_left, true);
    set_label_text_if_changed(&row.footer_left, footer_left);

    let action_count = visible_action_count(notification, data.is_active);
    let footer_right = if action_count == 0 {
        Cow::Borrowed("")
    } else {
        Cow::Owned(format!("{action_count} ACTIONS"))
    };
    set_label_visible_if_changed(&row.footer_right, !footer_right.is_empty());
    set_label_text_if_changed(&row.footer_right, footer_right.as_ref());
}

pub(super) fn notification_meta_label(notification: &NotificationView) -> String {
    match notification.urgency {
        value if value == Urgency::Critical as u8 => "ALERT".to_string(),
        value if value == Urgency::Low as u8 => "LOW".to_string(),
        _ => "NOTICE".to_string(),
    }
}

pub(super) fn relative_time_badge(received_at_ms: i64) -> String {
    if received_at_ms <= 0 {
        return String::new();
    }
    let Some(now_ms) = now_millis() else {
        return String::new();
    };
    let age_ms = now_ms.saturating_sub(received_at_ms.max(0) as u128);
    let age_secs = age_ms / 1_000;
    match age_secs {
        0..=59 => "now".to_string(),
        60..=3_599 => format!("{}m", age_secs / 60),
        3_600..=86_399 => format!("{}h", age_secs / 3_600),
        _ => format!("{}d", age_secs / 86_400),
    }
}

fn now_millis() -> Option<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

pub(super) fn notification_has_thumbnail(notification: &NotificationView) -> bool {
    notification.image.has_image_data || !notification.image.image_path.trim().is_empty()
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

fn set_widget_visible_if_changed<W: IsA<gtk::Widget>>(widget: &W, visible: bool) {
    // Stack ghost visibility can be replayed often while grouped counts change
    if widget.get_visible() != visible {
        widget.set_visible(visible);
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
    row: &NotificationRowWidgets,
    command_tx: &mpsc::Sender<UiCommand>,
    notification: &NotificationView,
    is_active: bool,
) {
    configure_inline_reply(
        &row.inline_reply,
        notification.id,
        &notification.inline_reply,
        is_active,
    );
    // Fast path: skip button rebuild when the action set is unchanged
    // This avoids tearing down buttons during no-op refresh passes
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
        // Reserve once so the cache grows with the current action count
        let mut cached = row.action_cache.borrow_mut();
        cached.clear();
        cached.reserve(notification.actions.len());
        for action in &notification.actions {
            cached.push((action.key.clone(), action.label.clone()));
        }
        row.action_cache_id.set(notification.id);
        *row.reply_cache.borrow_mut() = (notification.inline_reply.clone(), is_active);
    }

    // Refresh action buttons only when the action list changes
    while let Some(child) = row.actions_box.first_child() {
        // Remove old buttons before rebuilding the new set
        row.actions_box.remove(&child);
    }
    if visible_action_count(notification, is_active) == 0 {
        // No buttons should remain when the sender drops all actions
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
        row.actions_box.append(&button);
    }
}

fn visible_action_count(notification: &NotificationView, is_active: bool) -> usize {
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
