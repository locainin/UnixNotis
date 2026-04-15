//! Panel header construction

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::css::hooks;

use super::actions::{build_panel_actions, PanelActionWidgets};
use super::search::{build_panel_search, PanelSearchWidgets};

pub(super) struct PanelHeaderWidgets {
    pub(super) root: gtk::Box,
    pub(super) count: gtk::Label,
    pub(super) search: PanelSearchWidgets,
    pub(super) actions: PanelActionWidgets,
}

pub(super) fn build_panel_header() -> PanelHeaderWidgets {
    let header = gtk::Box::new(gtk::Orientation::Vertical, 8);
    header.add_css_class(hooks::panel_shell::HEADER);

    // Top row stays compact so header width does not jump across themes
    let header_top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header_top.add_css_class(hooks::panel_shell::HEADER_TOP);

    let title_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    title_box.add_css_class(hooks::panel_shell::TITLE_STACK);

    let title = gtk::Label::new(Some("Notifications"));
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

    let action_area = build_panel_actions();

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    // Spacer absorbs the flexible width between the title stack and close action
    spacer.set_hexpand(true);

    header_top.append(&title_box);
    header_top.append(&spacer);
    // Keep close away from clear so destructive actions do not blend together
    header_top.append(&action_area.widgets.close_button);
    header.append(&header_top);
    // Action row sits below the title so narrow panels stay stable
    header.append(&action_area.row);

    let search = build_panel_search();
    header.append(&search.revealer);

    PanelHeaderWidgets {
        root: header,
        count,
        search,
        actions: action_area.widgets,
    }
}
