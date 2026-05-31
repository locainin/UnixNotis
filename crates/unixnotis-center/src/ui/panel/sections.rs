//! Panel widget stack and scroller construction

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{
    css::hooks, PanelClearButtonPlacement, PanelConfig, PanelSection, PanelWidgetSection,
};

use super::actions::build_clear_button;

pub(crate) const WIDGET_REVEAL_TRANSITION_MS: u64 = 180;

pub(super) struct PanelSectionWidgets {
    pub(super) body_stack: gtk::Box,
    pub(super) widget_revealer: gtk::Revealer,
    pub(super) widget_stack: gtk::Box,
    pub(super) quick_controls: gtk::Box,
    pub(super) toggle_container: gtk::Box,
    pub(super) stat_container: gtk::Box,
    pub(super) card_container: gtk::Box,
    pub(super) scroller: gtk::ScrolledWindow,
    pub(super) notification_container: gtk::Box,
    pub(super) notification_header_row: gtk::Box,
    pub(super) notification_header: gtk::Label,
    pub(super) clear_header_button: gtk::Button,
    pub(super) toggle_section_header: gtk::Label,
    pub(super) stat_section_header: gtk::Label,
    pub(super) footer: gtk::Label,
    pub(super) media_container: gtk::Box,
}

pub(super) fn build_panel_sections(config: &PanelConfig) -> PanelSectionWidgets {
    let body_stack = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body_stack.add_css_class(hooks::panel_shell::BODY_STACK);
    body_stack.set_hexpand(true);
    body_stack.set_vexpand(true);

    let media_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
    media_container.add_css_class(hooks::panel_shell::MEDIA_CONTAINER);
    media_container.set_hexpand(true);
    media_container.set_halign(Align::Fill);

    let quick_controls = gtk::Box::new(gtk::Orientation::Vertical, 10);
    quick_controls.add_css_class(hooks::panel_shell::QUICK_CONTROLS);

    // Empty section boxes stay mounted so later inserts do not need a new container
    let (toggle_container, toggle_section_header) = build_section_box(
        hooks::panel_shell::TOGGLE_SECTION,
        &config.quick_actions_label,
    );
    let (stat_container, stat_section_header) = build_section_box(
        hooks::panel_shell::STAT_SECTION,
        &config.system_status_label,
    );
    let card_container = build_plain_section_box(hooks::panel_shell::CARD_SECTION);

    let widget_stack = gtk::Box::new(gtk::Orientation::Vertical, 8);
    widget_stack.add_css_class(hooks::panel_shell::WIDGET_STACK);
    for section in &config.widget_order {
        match section {
            PanelWidgetSection::Media => widget_stack.append(&media_container),
            PanelWidgetSection::Toggles => widget_stack.append(&toggle_container),
            PanelWidgetSection::Sliders => widget_stack.append(&quick_controls),
            PanelWidgetSection::Stats => widget_stack.append(&stat_container),
            PanelWidgetSection::Cards => widget_stack.append(&card_container),
        }
    }

    let widget_revealer = gtk::Revealer::new();
    widget_revealer.add_css_class(hooks::panel_shell::WIDGET_REVEALER);
    widget_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    widget_revealer.set_transition_duration(WIDGET_REVEAL_TRANSITION_MS as u32);
    widget_revealer.set_reveal_child(true);
    // Keep child widgets mounted so collapse does not rebuild stateful controls
    widget_revealer.set_child(Some(&widget_stack));

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(config.notification_list_expand);
    scroller.set_hexpand(true);
    scroller.set_halign(Align::Fill);
    // Keep vertical scrollbar width stable so the panel width does not jitter on hover
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Always);
    // The fixed panel owns width; the nested list must fill that space without
    // asking for the whole outer panel width from inside CSS padding/margins
    scroller.set_overlay_scrolling(false);

    let notification_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if config.notification_section_visible {
        notification_container.add_css_class(hooks::panel_shell::RECENT_SECTION);
    }
    notification_container.set_hexpand(true);
    notification_container.set_vexpand(config.notification_list_expand);
    notification_container.set_halign(Align::Fill);

    let recent_header_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    recent_header_row.add_css_class(hooks::panel_shell::RECENT_HEADER_ROW);
    recent_header_row.set_hexpand(true);
    recent_header_row.set_halign(Align::Fill);
    let recent_header = build_section_header(&config.recent_notifications_label);
    recent_header.add_css_class(hooks::panel_shell::RECENT_HEADER);
    recent_header.set_visible(
        config.notification_section_visible && !config.recent_notifications_label.is_empty(),
    );
    let recent_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    recent_spacer.set_hexpand(true);
    let clear_header_button = build_clear_button(config);
    clear_header_button.set_visible(matches!(
        config.clear_button_placement,
        PanelClearButtonPlacement::NotificationHeader
    ));
    recent_header_row.set_visible(notification_header_row_visible(config));
    recent_header_row.append(&recent_header);
    recent_header_row.append(&recent_spacer);
    recent_header_row.append(&clear_header_button);
    notification_container.append(&recent_header_row);
    notification_container.append(&scroller);
    append_panel_body_sections(
        &body_stack,
        &widget_revealer,
        &notification_container,
        &config.section_order,
    );

    let footer = gtk::Label::new(Some(&config.footer_label));
    footer.add_css_class(hooks::panel_shell::FOOTER);
    footer.set_xalign(0.5);
    footer.set_visible(!config.footer_label.is_empty());

    PanelSectionWidgets {
        body_stack,
        widget_revealer,
        widget_stack,
        quick_controls,
        toggle_container,
        stat_container,
        card_container,
        scroller,
        notification_container,
        notification_header_row: recent_header_row,
        notification_header: recent_header,
        clear_header_button,
        toggle_section_header,
        stat_section_header,
        footer,
        media_container,
    }
}

pub(in crate::ui::panel) fn apply_panel_body_section_order(
    body_stack: &gtk::Box,
    widget_revealer: &gtk::Revealer,
    notification_container: &gtk::Box,
    order: &[PanelSection],
) {
    let mut previous: Option<gtk::Widget> = None;
    for section in order {
        let child: gtk::Widget = match section {
            PanelSection::Widgets => widget_revealer.clone().upcast(),
            PanelSection::Notifications => notification_container.clone().upcast(),
        };
        body_stack.reorder_child_after(&child, previous.as_ref());
        previous = Some(child);
    }
}

fn append_panel_body_sections(
    body_stack: &gtk::Box,
    widget_revealer: &gtk::Revealer,
    notification_container: &gtk::Box,
    order: &[PanelSection],
) {
    for section in order {
        match section {
            PanelSection::Widgets => body_stack.append(widget_revealer),
            PanelSection::Notifications => body_stack.append(notification_container),
        }
    }
}

fn build_section_box(class_name: &str, label: &str) -> (gtk::Box, gtk::Label) {
    let section = build_plain_section_box(class_name);
    let header = build_section_header(label);
    section.append(&header);
    (section, header)
}

fn build_plain_section_box(class_name: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    section.add_css_class(class_name);
    section.set_hexpand(true);
    // Hidden-by-default keeps empty sections out of the layout until content appears
    section.set_visible(false);
    section
}

fn build_section_header(label: &str) -> gtk::Label {
    let header = gtk::Label::new(Some(label));
    header.add_css_class(hooks::panel_shell::SECTION_HEADER);
    header.set_xalign(0.0);
    header.set_visible(!label.is_empty());
    header
}

pub(crate) fn notification_header_row_visible(config: &PanelConfig) -> bool {
    matches!(
        config.clear_button_placement,
        PanelClearButtonPlacement::NotificationHeader
    ) || (config.notification_section_visible && !config.recent_notifications_label.is_empty())
}

#[cfg(test)]
#[path = "tests/sections.rs"]
mod tests;
