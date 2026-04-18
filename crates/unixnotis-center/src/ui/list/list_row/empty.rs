//! Empty-state row for the notification list
//!
//! This keeps the placeholder widget small and separate from the reusable rows

use gtk::prelude::*;
use gtk::{self, Align};
use unixnotis_core::css::hooks;

pub(in crate::ui::list) fn build_empty_row(text: &str) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class(hooks::empty_row::ROOT);
    root.set_hexpand(true);
    root.set_vexpand(true);
    root.set_valign(Align::Center);
    root.set_halign(Align::Center);

    let label = gtk::Label::new(Some(text));
    label.add_css_class(hooks::empty_row::LABEL);
    label.set_halign(Align::Center);
    label.set_valign(Align::Center);
    // Rely on explicit line breaks from config to avoid auto-hyphenation
    label.set_wrap(false);
    label.set_justify(gtk::Justification::Center);
    root.append(&label);

    root
}

pub(in crate::ui::list) fn update_empty_row(root: &gtk::Box, text: &str) {
    if let Some(child) = root.first_child() {
        if let Ok(label) = child.downcast::<gtk::Label>() {
            // Config reloads can change the empty-state copy without rebuilding the row
            // Skip the setter when the placeholder text already matches
            // This keeps config reloads from poking GTK for no visible change
            if label.text().as_str() != text {
                label.set_text(text);
            }
        }
    }
}
