//! Panel search row construction

use gtk::prelude::*;
use unixnotis_core::{css::hooks, PanelConfig};

pub(crate) const SEARCH_REVEAL_TRANSITION_MS: u64 = 180;

pub(super) struct PanelSearchWidgets {
    pub(super) revealer: gtk::Revealer,
    pub(super) entry: gtk::SearchEntry,
}

pub(super) fn build_panel_search(config: &PanelConfig) -> PanelSearchWidgets {
    let search_shell = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    search_shell.add_css_class(hooks::panel_shell::SEARCH_SHELL);
    search_shell.set_hexpand(true);

    let leading_accent = gtk::Box::new(gtk::Orientation::Vertical, 0);
    leading_accent.add_css_class(hooks::panel_shell::SEARCH_ACCENT);
    leading_accent.add_css_class(hooks::panel_shell::TICK_TOP_LEFT);

    let star_accent = gtk::Label::new(Some("*"));
    star_accent.add_css_class(hooks::panel_shell::SEARCH_STAR);

    let search_entry = gtk::SearchEntry::new();
    search_entry.add_css_class(hooks::panel_shell::SEARCH);
    // Placeholder text keeps the intent obvious before the first query
    search_entry.set_placeholder_text(Some(&config.search_placeholder));
    search_entry.set_hexpand(true);
    search_entry.set_tooltip_text(Some("Type to filter notifications"));
    search_shell.append(&leading_accent);
    search_shell.append(&search_entry);
    search_shell.append(&star_accent);

    let search_revealer = gtk::Revealer::new();
    search_revealer.add_css_class(hooks::panel_shell::SEARCH_REVEALER);
    // Slide-down matches the rest of the panel reveal motion
    search_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    search_revealer.set_transition_duration(SEARCH_REVEAL_TRANSITION_MS as u32);
    // Keep search hidden until the user asks for it so notifications keep the space
    search_revealer.set_reveal_child(config.search_visible);
    search_revealer.set_child(Some(&search_shell));

    PanelSearchWidgets {
        revealer: search_revealer,
        entry: search_entry,
    }
}
