//! Window construction and layout helpers for the popup surface.
//!
//! Keeps layout configuration isolated from popup state logic.

use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use gtk::{cairo, gdk};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use unixnotis_core::{Anchor, Config, Margins};

pub(super) fn build_popup_window(
    app: &gtk::Application,
    config: &Config,
) -> (gtk::ApplicationWindow, gtk::Box) {
    let window = gtk::ApplicationWindow::new(app);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_title(Some("UnixNotis Popups"));
    window.add_css_class("unixnotis-popup-window");

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);

    let stack = gtk::Box::new(gtk::Orientation::Vertical, config.popups.spacing);
    stack.add_css_class("unixnotis-popup-stack");
    window.set_child(Some(&stack));
    window.set_visible(false);
    apply_popup_config(&window, &stack, config);
    window.connect_realize({
        let allow_click_through = config.popups.allow_click_through;
        move |window| {
            apply_input_region(window, allow_click_through);
        }
    });

    (window, stack)
}

pub(super) fn apply_popup_config(
    window: &gtk::ApplicationWindow,
    stack: &gtk::Box,
    config: &Config,
) {
    window.set_default_size(config.popups.width, 1);
    window.set_size_request(config.popups.width, -1);
    stack.set_spacing(config.popups.spacing);

    apply_anchor(window, config.popups.anchor, config.popups.margin);
    window.set_exclusive_zone(0);
    window.set_keyboard_mode(KeyboardMode::None);

    if let Some(output) = config.popups.output.as_ref() {
        if let Some(monitor) = find_monitor(output) {
            window.set_monitor(Some(&monitor));
        }
    } else {
        window.set_monitor(None);
    }
    apply_input_region(window, config.popups.allow_click_through);
}

fn apply_input_region(window: &gtk::ApplicationWindow, allow_click_through: bool) {
    let Some(surface) = window.surface() else {
        return;
    };

    if allow_click_through {
        let region = cairo::Region::create();
        surface.set_input_region(&region);
        return;
    }

    // Clear the input region so the popup surface accepts clicks as normal.
    unsafe {
        gdk::ffi::gdk_surface_set_input_region(surface.to_glib_none().0, std::ptr::null_mut());
    }
}

fn apply_anchor(window: &impl IsA<gtk::Window>, anchor: Anchor, margin: Margins) {
    for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
        window.set_anchor(edge, false);
    }
    match anchor {
        Anchor::TopRight => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::TopLeft => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
        }
        Anchor::BottomRight => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::BottomLeft => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
        }
        Anchor::Top => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::Bottom => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
        }
        Anchor::Left => {
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);
        }
        Anchor::Right => {
            window.set_anchor(Edge::Right, true);
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);
        }
    }

    window.set_margin(Edge::Top, margin.top);
    window.set_margin(Edge::Right, margin.right);
    window.set_margin(Edge::Bottom, margin.bottom);
    window.set_margin(Edge::Left, margin.left);
}

fn find_monitor(name: &str) -> Option<gtk::gdk::Monitor> {
    let display = gtk::gdk::Display::default()?;
    let monitors = display.monitors();
    for index in 0..monitors.n_items() {
        let item = monitors.item(index)?;
        let monitor = item.downcast::<gtk::gdk::Monitor>().ok()?;
        if let Some(model) = monitor.model() {
            if model == name {
                return Some(monitor);
            }
        }
    }
    None
}
