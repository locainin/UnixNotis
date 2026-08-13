//! Panel window construction
//!
//! Window setup lives here while sizing and monitor helpers stay in their own modules

use gtk::prelude::*;
use gtk4_layer_shell::{Layer, LayerShell};
use unixnotis_core::{css::hooks, Config};

use super::body::build_panel_sections;
use super::header::build_panel_header;
use super::notice::build_reload_notice;
use super::widgets::PanelWidgets;

pub fn build_panel_widgets(app: &gtk::Application, config: &Config) -> PanelWidgets {
    let window = gtk::ApplicationWindow::new(app);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_title(Some("UnixNotis Center"));
    window.add_css_class(hooks::panel_shell::WINDOW);

    window.init_layer_shell();
    window.set_namespace(Some("unixnotis-panel"));
    window.set_layer(Layer::Overlay);
    super::geometry::apply_anchor(&window, config.panel.anchor, config.panel.margin);
    window.set_exclusive_zone(0);
    window.set_keyboard_mode(super::geometry::map_keyboard_mode(
        config.panel.keyboard_interactivity,
    ));

    let monitor = if let Some(output) = config.panel.output.as_ref() {
        // Named outputs fall back to the compositor default when the monitor disappears
        super::geometry::monitor::find_monitor(output)
            .or_else(super::geometry::monitor::default_monitor)
    } else {
        super::geometry::monitor::default_monitor()
    };
    if let Some(monitor) = monitor.as_ref() {
        window.set_monitor(Some(monitor));
    }

    let (width, height) = super::geometry::resolve_panel_size(config, monitor.as_ref(), None);
    // Default size guides the compositor while size request constrains GTK children
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
    let reload_notice = build_reload_notice();
    let sections = build_panel_sections(&config.panel, config.widgets.density);
    // Body chrome wraps the content without becoming part of configurable section ordering
    let body_chrome = build_panel_body_chrome(&sections.body_stack);
    let overlay = gtk::Overlay::new();
    overlay.set_hexpand(false);
    overlay.set_vexpand(true);
    // The overlay is the real window child, so it must carry the same width
    // request as the root panel box
    overlay.set_size_request(width, -1);
    overlay.set_child(Some(&root));

    // Chrome nodes intentionally carry no behavior
    // Themes can turn them into rails, corner ticks, or hidden no-op nodes
    // Overlay-only edge chrome avoids adding GTK box spacing to compact themes
    append_panel_edge_chrome(&overlay, true);
    root.append(&header.root);
    root.append(&reload_notice.revealer);
    root.append(&body_chrome);
    root.append(&sections.footer);
    append_panel_edge_chrome(&overlay, false);

    window.set_child(Some(&overlay));
    window.set_visible(false);

    let panel = PanelWidgets {
        window,
        surface: overlay,
        root,
        header,
        sections,
        reload_notice,
    };
    // Apply motion after construction so every long-lived revealer receives the same policy
    super::motion::apply_reduced_motion(&panel, config.panel.reduced_motion);
    panel
}

fn build_panel_body_chrome(body_stack: &gtk::Box) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    // Rails flank the only expanding body child and can remain visually empty
    body.set_hexpand(true);
    body.set_vexpand(true);

    let left_rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    left_rail.add_css_class(hooks::panel_shell::RAIL_LEFT);

    let right_rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    right_rail.add_css_class(hooks::panel_shell::RAIL_RIGHT);

    body.append(&left_rail);
    body.append(body_stack);
    body.append(&right_rail);
    body
}

fn append_panel_edge_chrome(overlay: &gtk::Overlay, top: bool) {
    let edge = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    // Overlay placement keeps decorative edges outside normal box measurement
    edge.set_hexpand(true);
    edge.set_halign(gtk::Align::Fill);
    if top {
        // Shared edge and corner hooks let themes draw one continuous top treatment
        edge.add_css_class(hooks::panel_shell::EDGE_TOP);
        edge.add_css_class(hooks::panel_shell::TICK_TOP_LEFT);
        edge.add_css_class(hooks::panel_shell::TICK_TOP_RIGHT);
        edge.set_valign(gtk::Align::Start);
    } else {
        // Bottom hooks mirror the top without requiring a second layout implementation
        edge.add_css_class(hooks::panel_shell::EDGE_BOTTOM);
        edge.add_css_class(hooks::panel_shell::TICK_BOTTOM_LEFT);
        edge.add_css_class(hooks::panel_shell::TICK_BOTTOM_RIGHT);
        edge.set_valign(gtk::Align::End);
    }
    overlay.add_overlay(&edge);
}
