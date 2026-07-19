//! Bounded notification label state and GTK updates

use std::borrow::Cow;

use gtk::prelude::*;

use super::super::state::{
    NotificationRowWidgets, OptionalLabelState, MAX_BODY_LABEL_CHARS, MAX_SUMMARY_LABEL_CHARS,
};

pub(super) fn update_notification_text(
    row: &NotificationRowWidgets,
    app_name: &str,
    summary: &str,
    body: &str,
) {
    // App name always renders while optional rows collapse on empty text
    set_label_text_if_changed(&row.app_label, app_name);
    update_optional_label(&row.summary_label, summary, MAX_SUMMARY_LABEL_CHARS);
    update_optional_label(&row.body_label, body, MAX_BODY_LABEL_CHARS);
}

pub(super) fn optional_label_state(text: &str, max_chars: usize) -> OptionalLabelState<'_> {
    if !has_visible_text(text) || max_chars == 0 {
        // Empty and intentionally blanked labels must not reserve row space
        return OptionalLabelState {
            visible: false,
            text: Cow::Borrowed(""),
        };
    }
    OptionalLabelState {
        visible: true,
        // Notification text stays plain so markup cannot change the layout
        text: clamp_label_text(text, max_chars),
    }
}

fn update_optional_label(label: &gtk::Label, text: &str, max_chars: usize) {
    // Summary and body use one hide-or-clamp rule
    let state = optional_label_state(text, max_chars);
    set_label_visible_if_changed(label, state.visible);
    set_label_text_if_changed(label, state.text.as_ref());
}

pub(super) fn has_visible_text(text: &str) -> bool {
    // Layout only needs to know whether real visible content exists
    text.chars().any(|ch| !ch.is_whitespace())
}

pub(super) fn set_label_visible_if_changed(label: &gtk::Label, visible: bool) {
    // Reused rows often receive the same visibility decision
    if label.get_visible() != visible {
        label.set_visible(visible);
    }
}

pub(super) fn set_label_text_if_changed(label: &gtk::Label, text: &str) {
    // GTK only needs real text changes
    if label.text().as_str() != text {
        label.set_text(text);
    }
}

pub(super) fn clamp_label_text(text: &str, max_chars: usize) -> Cow<'_, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }
    // Character boundaries keep UTF-8 valid after truncation
    for (chars, (idx, _)) in text.char_indices().enumerate() {
        if chars == max_chars {
            let mut clamped = String::with_capacity(idx + 3);
            clamped.push_str(&text[..idx]);
            clamped.push('…');
            return Cow::Owned(clamped);
        }
    }
    Cow::Borrowed(text)
}
