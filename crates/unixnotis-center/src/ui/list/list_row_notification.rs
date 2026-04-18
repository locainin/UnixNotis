//! Notification row widget construction and updates.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::pango::{EllipsizeMode, WrapMode};
use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::{hooks, NotificationView, Urgency};

use crate::dbus::UiCommand;

use super::super::icons::IconResolver;
use super::super::try_send_command;
use super::list_item::RowData;

pub(super) struct NotificationRowWidgets {
    // Main icon shown at the top-left of the row
    pub(super) icon: gtk::Image,
    // App name text shown beside the icon
    pub(super) app_label: gtk::Label,
    // Summary line with stronger visual weight
    pub(super) summary_label: gtk::Label,
    // Body text section that can span multiple lines
    pub(super) body_label: gtk::Label,
    // Container for optional action buttons
    pub(super) actions_box: gtk::Box,
    // Current notification id bound to this reused row widget
    pub(super) notify_id: Rc<Cell<u32>>,
    // Last rendered action signature for cheap no-op detection
    pub(super) action_cache: RefCell<Vec<(String, String)>>,
    // Last rendered icon signature so decoding only runs when needed
    pub(super) icon_sig: RefCell<Option<IconSignature>>,
}

// Hard caps keep very large payloads from blowing up row height
const MAX_SUMMARY_LABEL_CHARS: usize = 160;
const MAX_BODY_LABEL_CHARS: usize = 512;
// Action labels stay bounded so one button cannot distort the whole row
const MAX_ACTION_LABEL_CHARS: usize = 20;

struct OptionalLabelState<'a> {
    // Hidden rows should collapse instead of leaving dead card spacing
    visible: bool,
    // Borrow when possible so repeated row refreshes do not allocate
    text: Cow<'a, str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IconSignature {
    image_path: String,
    icon_name: String,
    app_name: String,
    has_image_data: bool,
    image_len: usize,
    image_width: i32,
    image_height: i32,
}

impl IconSignature {
    fn from(notification: &NotificationView) -> Self {
        // Signature includes all fields that can change icon resolution output
        Self {
            image_path: notification.image.image_path.clone(),
            icon_name: notification.image.icon_name.clone(),
            app_name: notification.app_name.clone(),
            has_image_data: notification.image.has_image_data,
            image_len: notification.image.image_data.data.len(),
            image_width: notification.image.image_data.width,
            image_height: notification.image.image_data.height,
        }
    }
}

pub(super) fn build_notification_row(
    command_tx: mpsc::Sender<UiCommand>,
) -> (gtk::Box, NotificationRowWidgets) {
    // Root card uses vertical layout: header, summary, body, then actions
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.add_css_class("unixnotis-panel-card");

    // Header packs icon + app label + close button
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let icon = gtk::Image::new();
    icon.set_pixel_size(22);
    icon.add_css_class("unixnotis-panel-icon");

    let app_label = gtk::Label::new(None);
    app_label.set_xalign(0.0);
    // Ellipsis avoids row width spikes from long app names
    app_label.set_ellipsize(EllipsizeMode::End);
    app_label.set_single_line_mode(true);
    app_label.set_max_width_chars(40);
    app_label.add_css_class("unixnotis-panel-app");

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    // Spacer pushes close button to the far edge
    spacer.set_hexpand(true);

    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.set_halign(gtk::Align::End);
    close_button.add_css_class("unixnotis-panel-close");

    header.append(&icon);
    header.append(&app_label);
    header.append(&spacer);
    header.append(&close_button);

    // Summary is optional, so the update path decides later if the row should exist
    let summary_label = gtk::Label::new(None);
    summary_label.set_xalign(0.0);
    // Summary can wrap but stays bounded to three lines
    summary_label.set_wrap(true);
    summary_label.set_wrap_mode(WrapMode::WordChar);
    summary_label.set_ellipsize(EllipsizeMode::End);
    summary_label.set_lines(3);
    summary_label.set_max_width_chars(88);
    summary_label.add_css_class("unixnotis-panel-summary");

    // Body follows the same optional-row rule as summary text
    let body_label = gtk::Label::new(None);
    body_label.set_xalign(0.0);
    // Body gets more lines than summary but still has upper bounds
    body_label.set_wrap(true);
    body_label.set_wrap_mode(WrapMode::WordChar);
    body_label.set_ellipsize(EllipsizeMode::End);
    body_label.set_lines(8);
    body_label.set_max_width_chars(112);
    body_label.add_css_class("unixnotis-panel-body");

    let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    // Action buttons are added on demand during row updates
    actions_box.add_css_class("unixnotis-notification-actions");

    root.append(&header);
    root.append(&summary_label);
    root.append(&body_label);
    root.append(&actions_box);

    let notify_id = Rc::new(Cell::new(0));
    // Close click always targets the latest id assigned to this row
    let close_tx = command_tx.clone();
    let notify_id_clone = notify_id.clone();
    close_button.connect_clicked(move |_| {
        let id = notify_id_clone.get();
        if id == 0 {
            // Ignore clicks before first binding
            return;
        }
        debug!(id, "dismiss clicked");
        // Non-blocking enqueue avoids GTK stalls during D-Bus backpressure.
        try_send_command(&close_tx, UiCommand::Dismiss(id));
    });

    (
        root,
        NotificationRowWidgets {
            icon,
            app_label,
            summary_label,
            body_label,
            actions_box,
            notify_id,
            action_cache: RefCell::new(Vec::new()),
            icon_sig: RefCell::new(None),
        },
    )
}

pub(super) fn update_notification_row(
    row: &NotificationRowWidgets,
    root: &gtk::Box,
    data: &RowData,
    icon_resolver: &IconResolver,
    command_tx: &mpsc::Sender<UiCommand>,
) {
    // Recycled rows can be updated with None while model changes
    let Some(notification) = data.notification.as_ref() else {
        return;
    };
    let notification = notification.as_ref();

    if notification.urgency == Urgency::Critical as u8 {
        // CSS class toggles are explicit to avoid stale visual state
        set_class_state(root, hooks::shared_state::CRITICAL, true);
    } else {
        set_class_state(root, hooks::shared_state::CRITICAL, false);
    }
    if data.is_active {
        // Active rows can be styled differently from history rows
        set_class_state(root, hooks::shared_state::ACTIVE, true);
    } else {
        set_class_state(root, hooks::shared_state::ACTIVE, false);
    }
    if data.stacked {
        // Stacked class indicates collapsed entries in grouped mode
        set_class_state(root, hooks::shared_state::STACKED, true);
    } else {
        set_class_state(root, hooks::shared_state::STACKED, false);
    }

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

    // Icon decode/apply is skipped when signature is unchanged
    let next_sig = IconSignature::from(notification);
    let mut sig_guard = row.icon_sig.borrow_mut();
    if sig_guard.as_ref() != Some(&next_sig) {
        let scale = root.scale_factor();
        icon_resolver.apply_icon(&row.icon, notification, 22, scale);
        *sig_guard = Some(next_sig);
    }
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
    let state = optional_label_state(text, max_chars);
    set_label_visible_if_changed(label, state.visible);
    set_label_text_if_changed(label, state.text.as_ref());
}

fn optional_label_state(text: &str, max_chars: usize) -> OptionalLabelState<'_> {
    if !has_visible_text(text) {
        // Empty text rows stay hidden so card spacing stays honest
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

fn has_visible_text(text: &str) -> bool {
    // Layout only needs to know if the row has real visible content
    text.chars().any(|ch| !ch.is_whitespace())
}

fn set_class_state(root: &gtk::Box, class_name: &str, enabled: bool) {
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
    if label.is_visible() != visible {
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

fn clamp_action_label_text(text: &str) -> Cow<'_, str> {
    // Action text uses the same clamp rule every time so row width stays stable
    // This keeps the panel from being stretched by one bad button label
    clamp_label_text(text, MAX_ACTION_LABEL_CHARS)
}

fn clamp_label_text(text: &str, max_chars: usize) -> Cow<'_, str> {
    if max_chars == 0 {
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
    // Fast path: skip button rebuild when action set is unchanged
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
        // Cache current action signature for next update cycle
        let mut cached = cache.borrow_mut();
        cached.clear();
        cached.reserve(notification.actions.len());
        for action in &notification.actions {
            cached.push((action.key.clone(), action.label.clone()));
        }
    }

    // Refresh action buttons only when the action list changes.
    while let Some(child) = actions_box.first_child() {
        // Remove old buttons before rebuilding the new set
        actions_box.remove(&child);
    }
    if notification.actions.is_empty() {
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
        button.connect_clicked(move |_| {
            debug!(id, action = %action_key, "action invoked");
            // Action execution is best-effort and non-blocking
            // Best-effort enqueue keeps action handling responsive.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_summary_row_hides_when_text_is_empty() {
        // Empty summaries should not leave a blank strip above the body
        let state = optional_label_state("", MAX_SUMMARY_LABEL_CHARS);

        assert!(!state.visible);
        assert!(state.text.is_empty());
    }

    #[test]
    fn panel_summary_row_hides_when_text_is_only_whitespace() {
        // Space-only payloads should collapse the same as truly empty payloads
        let state = optional_label_state("\n\t ", MAX_SUMMARY_LABEL_CHARS);

        assert!(!state.visible);
        assert!(state.text.is_empty());
    }

    #[test]
    fn panel_summary_row_shows_when_text_has_real_content() {
        // Leading and trailing space should not hide actual notification text
        let state = optional_label_state("  hello  ", MAX_SUMMARY_LABEL_CHARS);

        assert!(state.visible);
        assert_eq!(state.text.as_ref(), "  hello  ");
    }

    #[test]
    fn panel_action_labels_are_clamped_before_button_build() {
        // Long labels should be shortened before the action row sees them
        let long_label = "This action label is much longer than the row should allow";
        let rendered = clamp_action_label_text(long_label);

        assert!(rendered.len() < long_label.len());
        assert!(rendered.ends_with('…'));
    }
}
