//! Panel header widget construction

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{css::hooks, PanelConfig};

use super::actions::{action_order_contains_close, build_panel_actions};
use super::search::build_panel_search;
use super::widgets::PanelHeaderWidgets;

pub(in crate::ui::panel) fn build_panel_header(config: &PanelConfig) -> PanelHeaderWidgets {
    let header = gtk::Box::new(gtk::Orientation::Vertical, 8);
    header.add_css_class(hooks::panel_shell::HEADER);

    // Top row stays compact so header width does not jump across themes
    let header_top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header_top.add_css_class(hooks::panel_shell::HEADER_TOP);

    let title_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    title_box.add_css_class(hooks::panel_shell::TITLE_STACK);

    let title = gtk::Label::new(Some(&config.title));
    title.set_xalign(0.0);
    title.add_css_class(hooks::panel_shell::TITLE);

    let count = gtk::Label::new(Some("0"));
    // Count stays centered so one and three digit values do not jump left
    count.set_xalign(0.5);
    count.set_valign(Align::Center);
    count.add_css_class(hooks::panel_shell::COUNT);

    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    title_row.add_css_class(hooks::panel_shell::TITLE_ROW);
    // Title and count stay in one row so the header can shrink cleanly
    title_row.append(&title);
    title_row.append(&count);
    title_box.append(&title_row);

    let subtitle = gtk::Label::new(Some(&config.subtitle));
    subtitle.set_xalign(0.0);
    subtitle.add_css_class(hooks::panel_shell::SUBTITLE);
    subtitle.set_visible(!config.subtitle.is_empty());
    title_box.append(&subtitle);

    let action_area = build_panel_actions(config);
    action_area.row.set_visible(config.action_row_visible);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    // Spacer absorbs the flexible width between the title stack and close action
    spacer.set_hexpand(true);

    header_top.append(&title_box);
    header_top.append(&spacer);
    if !action_order_contains_close(&config.action_order) {
        // Keep close away from clear so destructive actions do not blend together
        header_top.append(&action_area.widgets.close_button);
    }
    header.append(&header_top);
    // Action row sits below the title so narrow panels stay stable
    header.append(&action_area.row);

    let search = build_panel_search(config);
    // Initial configuration must keep the toggle aligned with the visible search row
    action_area
        .widgets
        .search_toggle
        .set_active(search.revealer.reveals_child());
    header.append(&search.revealer);

    PanelHeaderWidgets {
        root: header,
        top: header_top,
        action_row: action_area.row,
        title,
        subtitle,
        count,
        search,
        actions: action_area.widgets,
    }
}

#[cfg(test)]
#[path = "tests/build.rs"]
mod tests;
