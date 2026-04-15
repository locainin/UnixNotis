//! Panel search row construction

use gtk::prelude::*;
use unixnotis_core::css::hooks;

pub(super) struct PanelSearchWidgets {
    pub(super) revealer: gtk::Revealer,
    pub(super) entry: gtk::SearchEntry,
}

pub(super) fn build_panel_search() -> PanelSearchWidgets {
    let search_entry = gtk::SearchEntry::new();
    search_entry.add_css_class(hooks::panel_shell::SEARCH);
    // Placeholder text keeps the intent obvious before the first query
    search_entry.set_placeholder_text(Some("Search app, title, or message"));
    search_entry.set_hexpand(true);
    search_entry.set_tooltip_text(Some("Type to filter notifications"));

    let search_revealer = gtk::Revealer::new();
    search_revealer.add_css_class(hooks::panel_shell::SEARCH_REVEALER);
    // Slide-down matches the rest of the panel reveal motion
    search_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    search_revealer.set_transition_duration(180);
    // Keep search hidden until the user asks for it so notifications keep the space
    search_revealer.set_reveal_child(false);
    search_revealer.set_child(Some(&search_entry));

    PanelSearchWidgets {
        revealer: search_revealer,
        entry: search_entry,
    }
}
