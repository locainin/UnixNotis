//! Panel window construction
//!
//! Window setup lives here while sizing and monitor helpers stay in their own modules

use gtk::prelude::*;
use gtk4_layer_shell::{Layer, LayerShell};
use unixnotis_core::{css::hooks, Config};

use super::header::build_panel_header;
use super::sections::build_panel_sections;
use super::types::PanelWidgets;

pub fn build_panel_widgets(app: &gtk::Application, config: &Config) -> PanelWidgets {
    let window = gtk::ApplicationWindow::new(app);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_title(Some("UnixNotis Center"));
    window.add_css_class(hooks::panel_shell::WINDOW);
    if let Some(settings) = gtk::Settings::default() {
        // GTK global setting that controls whether scrollbars overlay content
        // Enabled here to keep scrollbar behavior consistent across widgets
        settings.set_property("gtk-overlay-scrolling", true);
    }

    window.init_layer_shell();
    window.set_namespace(Some("unixnotis-panel"));
    window.set_layer(Layer::Overlay);
    super::layout::apply_anchor(&window, config.panel.anchor, config.panel.margin);
    window.set_exclusive_zone(0);
    window.set_keyboard_mode(super::layout::map_keyboard_mode(
        config.panel.keyboard_interactivity,
    ));

    let monitor = if let Some(output) = config.panel.output.as_ref() {
        super::monitor::find_monitor(output).or_else(super::monitor::default_monitor)
    } else {
        super::monitor::default_monitor()
    };
    if let Some(monitor) = monitor.as_ref() {
        window.set_monitor(Some(monitor));
    }

    let (width, height) = super::layout::resolve_panel_size(config, monitor.as_ref(), None);
    window.set_default_size(width, height);
    if height > 0 {
        window.set_size_request(width, height);
    } else {
        window.set_size_request(width, -1);
    }

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.add_css_class(hooks::panel_shell::ROOT);
    root.set_focusable(true);
    root.set_hexpand(true);
    root.set_vexpand(true);
    // Keep the panel width stable regardless of child content
    root.set_size_request(width, -1);

    let header = build_panel_header(&config.panel);
    let sections = build_panel_sections(&config.panel);

    root.append(&header.root);
    root.append(&sections.widget_revealer);
    root.append(&sections.notification_container);
    root.append(&sections.footer);

    window.set_child(Some(&root));
    window.set_visible(false);

    PanelWidgets {
        window,
        root,
        widget_revealer: sections.widget_revealer,
        widget_stack: sections.widget_stack,
        quick_controls: sections.quick_controls,
        toggle_container: sections.toggle_container,
        stat_container: sections.stat_container,
        card_container: sections.card_container,
        scroller: sections.scroller,
        media_container: sections.media_container,
        search_revealer: header.search.revealer,
        search_entry: header.search.entry,
        search_toggle: header.actions.search_toggle,
        header_title: header.title,
        header_subtitle: header.subtitle,
        header_count: header.count,
        header_action_row: header.action_row,
        notification_container: sections.notification_container,
        notification_header_row: sections.notification_header_row,
        notification_header: sections.notification_header,
        toggle_section_header: sections.toggle_section_header,
        stat_section_header: sections.stat_section_header,
        footer_label: sections.footer,
        focus_toggle: header.actions.focus_toggle,
        dnd_toggle: header.actions.dnd_toggle,
        clear_action_button: header.actions.clear_button,
        clear_header_button: sections.clear_header_button,
        close_button: header.actions.close_button,
    }
}
