//! Bounded text and error presentation for inline replies

use std::borrow::Cow;

use gtk::prelude::*;
use unixnotis_core::util;

pub(super) const DEFAULT_PLACEHOLDER: &str = "Type a reply…";
pub(super) const DEFAULT_SUBMIT_LABEL: &str = "Send";

// Button text stays compact even when the sender provides a long custom hint
const MAX_SUBMIT_LABEL_CHARS: usize = 20;
const MAX_REPLY_ERROR_CHARS: usize = 180;
const APPLICATION_UNAVAILABLE: &str = "The application is no longer available";

pub(super) fn clear_reply_error(label: &gtk::Label) {
    label.set_text("");
    label.set_visible(false);
}

pub(super) fn show_reply_error(label: &gtk::Label, error: &str) {
    // Known liveness failures use a short stable message instead of a D-Bus error prefix
    let message = if error.contains(APPLICATION_UNAVAILABLE) {
        APPLICATION_UNAVAILABLE.to_string()
    } else {
        util::sanitize_inline_display_text(error)
    };
    let message = clamp_error_message(&message);
    label.set_text(&format!("Could not send: {message}"));
    label.set_visible(true);
}

fn clamp_error_message(message: &str) -> Cow<'_, str> {
    // Remote error text is display-only and must not create an unbounded row
    let Some((cut, _)) = message.char_indices().nth(MAX_REPLY_ERROR_CHARS) else {
        return Cow::Borrowed(message);
    };
    let mut bounded = String::with_capacity(cut + 3);
    bounded.push_str(&message[..cut]);
    bounded.push('…');
    Cow::Owned(bounded)
}

pub(super) fn update_submit_content(button: &gtk::Button, label: &str, icon_name: &str) {
    // Rebuild the tiny child box because KDE may change hints on replacement
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    if !icon_name.is_empty() {
        let icon = gtk::Image::from_icon_name(icon_name);
        content.append(&icon);
    }
    let label = if label.is_empty() {
        DEFAULT_SUBMIT_LABEL
    } else {
        label
    };
    let label = gtk::Label::new(Some(clamp_submit_label(label).as_ref()));
    content.append(&label);
    button.set_child(Some(&content));
}

fn clamp_submit_label(label: &str) -> Cow<'_, str> {
    // Character indexes preserve UTF-8 boundaries while enforcing visual length
    let Some((cut, _)) = label.char_indices().nth(MAX_SUBMIT_LABEL_CHARS) else {
        return Cow::Borrowed(label);
    };
    let mut bounded = String::with_capacity(cut + 3);
    bounded.push_str(&label[..cut]);
    bounded.push('…');
    Cow::Owned(bounded)
}
