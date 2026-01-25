//! Empty-state row for the notification list.
//!
//! Keeps the "no notifications" placeholder in one place so styling stays consistent.

use gtk::prelude::*;
use gtk::{self, Align};

pub(super) fn build_empty_row(text: &str) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("unixnotis-empty");
    root.set_hexpand(true);
    root.set_vexpand(true);
    root.set_valign(Align::Center);
    root.set_halign(Align::Center);

    let label = gtk::Label::new(Some(text));
    label.add_css_class("unixnotis-empty-label");
    label.set_halign(Align::Center);
    label.set_valign(Align::Center);
    // Rely on explicit line breaks from config to avoid auto-hyphenation.
    label.set_wrap(false);
    label.set_justify(gtk::Justification::Center);
    root.append(&label);

    root
}

pub(super) fn update_empty_row(root: &gtk::Box, text: &str) {
    if let Some(child) = root.first_child() {
        if let Ok(label) = child.downcast::<gtk::Label>() {
            // Keep the empty-state label in sync with config reloads.
            label.set_text(text);
        }
    }
}
