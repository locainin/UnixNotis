//! Panel widget stack and scroller construction

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::css::hooks;

pub(super) struct PanelSectionWidgets {
    pub(super) widget_revealer: gtk::Revealer,
    pub(super) quick_controls: gtk::Box,
    pub(super) toggle_container: gtk::Box,
    pub(super) stat_container: gtk::Box,
    pub(super) card_container: gtk::Box,
    pub(super) scroller: gtk::ScrolledWindow,
    pub(super) media_container: gtk::Box,
}

pub(super) fn build_panel_sections(width: i32) -> PanelSectionWidgets {
    let media_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
    media_container.add_css_class(hooks::panel_shell::MEDIA_CONTAINER);
    media_container.set_hexpand(true);
    media_container.set_halign(Align::Fill);

    let quick_controls = gtk::Box::new(gtk::Orientation::Vertical, 10);
    quick_controls.add_css_class(hooks::panel_shell::QUICK_CONTROLS);

    // Empty section boxes stay mounted so later inserts do not need a new container
    let toggle_container = build_section_box(hooks::panel_shell::TOGGLE_SECTION);
    let stat_container = build_section_box(hooks::panel_shell::STAT_SECTION);
    let card_container = build_section_box(hooks::panel_shell::CARD_SECTION);

    let widget_stack = gtk::Box::new(gtk::Orientation::Vertical, 8);
    widget_stack.add_css_class(hooks::panel_shell::WIDGET_STACK);
    // Stack order matches the visible panel layout from top to bottom
    widget_stack.append(&quick_controls);
    widget_stack.append(&media_container);
    widget_stack.append(&toggle_container);
    widget_stack.append(&stat_container);
    widget_stack.append(&card_container);

    let widget_revealer = gtk::Revealer::new();
    widget_revealer.add_css_class(hooks::panel_shell::WIDGET_REVEALER);
    widget_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    widget_revealer.set_transition_duration(180);
    widget_revealer.set_reveal_child(true);
    // Keep child widgets mounted so collapse does not rebuild stateful controls
    widget_revealer.set_child(Some(&widget_stack));

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_hexpand(true);
    // Keep vertical scrollbar width stable so the panel width does not jitter on hover
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Always);
    // Overlay scrollbars can steal width from a fixed-width panel
    scroller.set_overlay_scrolling(false);
    scroller.set_min_content_width(width);
    scroller.set_max_content_width(width);

    PanelSectionWidgets {
        widget_revealer,
        quick_controls,
        toggle_container,
        stat_container,
        card_container,
        scroller,
        media_container,
    }
}

fn build_section_box(class_name: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    section.add_css_class(class_name);
    section.set_hexpand(true);
    // Hidden-by-default keeps empty sections out of the layout until content appears
    section.set_visible(false);
    section
}
