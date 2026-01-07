//! Panel layout and widget construction for the center window.

use gtk::gdk;
use gtk::gdk::prelude::*;
use gtk::prelude::*;
use gtk::Align;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use unixnotis_core::{Anchor, Config, Margins, PanelKeyboardInteractivity};

/// GTK widgets backing the notification center panel window.
pub struct PanelWidgets {
    pub window: gtk::ApplicationWindow,
    pub root: gtk::Box,
    pub quick_controls: gtk::Box,
    pub toggle_container: gtk::Box,
    pub stat_container: gtk::Box,
    pub card_container: gtk::Box,
    pub scroller: gtk::ScrolledWindow,
    pub media_container: gtk::Box,
    pub header_count: gtk::Label,
    pub dnd_toggle: gtk::ToggleButton,
    pub clear_button: gtk::Button,
    pub close_button: gtk::Button,
}

pub fn build_panel_widgets(app: &gtk::Application, config: &Config) -> PanelWidgets {
    let window = gtk::ApplicationWindow::new(app);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_title(Some("UnixNotis Center"));
    window.add_css_class("unixnotis-panel-window");

    window.init_layer_shell();
    window.set_namespace(Some("unixnotis-panel"));
    window.set_layer(Layer::Overlay);
    apply_anchor(&window, config.panel.anchor, config.panel.margin);
    window.set_exclusive_zone(0);
    window.set_keyboard_mode(map_keyboard_mode(config.panel.keyboard_interactivity));

    let monitor = if let Some(output) = config.panel.output.as_ref() {
        find_monitor(output).or_else(default_monitor)
    } else {
        default_monitor()
    };
    if let Some(monitor) = monitor.as_ref() {
        window.set_monitor(Some(monitor));
    }

    let (width, height) = resolve_panel_size(config, monitor.as_ref(), None);
    window.set_default_size(width, height);
    if height > 0 {
        window.set_size_request(width, height);
    } else {
        window.set_size_request(width, -1);
    }

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.add_css_class("unixnotis-panel");
    root.set_focusable(true);
    root.set_hexpand(true);
    root.set_vexpand(true);
    // Keep the panel width stable regardless of child content.
    root.set_size_request(width, -1);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("unixnotis-panel-header");

    let title_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let title = gtk::Label::new(Some("Notifications"));
    title.set_xalign(0.0);
    title.add_css_class("unixnotis-panel-title");
    let count = gtk::Label::new(Some("0"));
    count.set_xalign(0.5);
    count.set_valign(Align::Center);
    count.add_css_class("unixnotis-panel-count");
    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    title_row.append(&title);
    title_row.append(&count);
    title_box.append(&title_row);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class("unixnotis-panel-actions");

    let dnd_toggle = gtk::ToggleButton::with_label("Do Not Disturb");
    dnd_toggle.add_css_class("unixnotis-panel-action");
    let clear_button = gtk::Button::with_label("Clear");
    clear_button.add_css_class("unixnotis-panel-action");
    let close_button = gtk::Button::with_label("Close");
    close_button.add_css_class("unixnotis-panel-action");

    actions.append(&dnd_toggle);
    actions.append(&clear_button);
    actions.append(&close_button);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    spacer.set_hexpand(true);
    // Spacer expands to align actions to the trailing edge.
    header.append(&title_box);
    header.append(&spacer);
    header.append(&actions);

    let media_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
    media_container.add_css_class("unixnotis-media-container");

    let quick_controls = gtk::Box::new(gtk::Orientation::Vertical, 10);
    quick_controls.add_css_class("unixnotis-quick-controls");

    let toggle_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    toggle_container.add_css_class("unixnotis-toggle-section");
    toggle_container.set_hexpand(true);
    toggle_container.set_visible(false);

    let stat_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    stat_container.add_css_class("unixnotis-stat-section");
    stat_container.set_hexpand(true);
    stat_container.set_visible(false);

    let card_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card_container.add_css_class("unixnotis-card-section");
    card_container.set_hexpand(true);
    card_container.set_visible(false);

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_hexpand(true);
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_min_content_width(width);
    scroller.set_max_content_width(width);

    root.append(&header);
    root.append(&quick_controls);
    root.append(&media_container);
    root.append(&toggle_container);
    root.append(&stat_container);
    root.append(&card_container);
    root.append(&scroller);

    window.set_child(Some(&root));
    window.set_visible(false);

    PanelWidgets {
        window,
        root,
        quick_controls,
        toggle_container,
        stat_container,
        card_container,
        scroller,
        media_container,
        header_count: count,
        dnd_toggle,
        clear_button,
        close_button,
    }
}

fn resolve_panel_size(
    config: &Config,
    monitor: Option<&gdk::Monitor>,
    reserved: Option<Margins>,
) -> (i32, i32) {
    let width = config.panel.width.max(1);
    if config.panel.height > 0 {
        return (width, config.panel.height);
    }
    if matches!(config.panel.anchor, Anchor::Left | Anchor::Right) {
        if let Some(height) = compute_side_panel_height(config, monitor, reserved) {
            return (width, height);
        }
    }
    // Natural height keeps top or bottom anchored panels compact when no explicit size is set.
    (width, -1)
}

fn compute_side_panel_height(
    config: &Config,
    monitor: Option<&gdk::Monitor>,
    reserved: Option<Margins>,
) -> Option<i32> {
    const MIN_HEIGHT: i32 = 520;
    const BOTTOM_PAD: i32 = 96;

    if !matches!(config.panel.anchor, Anchor::Left | Anchor::Right) {
        return None;
    }

    let monitor = monitor?;
    let geometry = monitor.geometry();
    let mut work_area = geometry.height() - (config.panel.margin.top + config.panel.margin.bottom);
    if config.panel.respect_work_area {
        if let Some(reserved) = reserved {
            work_area -= reserved.top + reserved.bottom;
        }
    }
    if work_area <= 0 {
        return None;
    }

    let max_height = (work_area - BOTTOM_PAD).max(1);
    let min_height = MIN_HEIGHT.min(max_height);

    // Keep the panel tall while leaving a small bottom gap.
    Some(max_height.max(min_height))
}

fn default_monitor() -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    let item = monitors.item(0)?;
    item.downcast::<gdk::Monitor>().ok()
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
            // Avoid bottom anchoring so computed height and overrides are respected.
        }
        Anchor::Right => {
            window.set_anchor(Edge::Right, true);
            window.set_anchor(Edge::Top, true);
            // Avoid bottom anchoring so computed height and overrides are respected.
        }
    }

    window.set_margin(Edge::Top, margin.top);
    window.set_margin(Edge::Right, margin.right);
    window.set_margin(Edge::Bottom, margin.bottom);
    window.set_margin(Edge::Left, margin.left);
}

pub fn apply_panel_config(panel: &PanelWidgets, config: &Config, reserved: Option<Margins>) {
    let monitor = if let Some(output) = config.panel.output.as_ref() {
        find_monitor(output).or_else(default_monitor)
    } else {
        default_monitor()
    };
    if let Some(monitor) = monitor.as_ref() {
        panel.window.set_monitor(Some(monitor));
    }

    panel
        .window
        .set_keyboard_mode(map_keyboard_mode(config.panel.keyboard_interactivity));
    apply_anchor(&panel.window, config.panel.anchor, config.panel.margin);

    let (width, height) = resolve_panel_size(config, monitor.as_ref(), reserved);
    panel.window.set_default_size(width, height);
    if height > 0 {
        panel.window.set_size_request(width, height);
    } else {
        panel.window.set_size_request(width, -1);
    }
    panel.root.set_size_request(width, -1);
    panel.scroller.set_min_content_width(width);
    panel.scroller.set_max_content_width(width);
}

fn map_keyboard_mode(mode: PanelKeyboardInteractivity) -> KeyboardMode {
    match mode {
        PanelKeyboardInteractivity::None => KeyboardMode::None,
        PanelKeyboardInteractivity::OnDemand => KeyboardMode::OnDemand,
        PanelKeyboardInteractivity::Exclusive => KeyboardMode::Exclusive,
    }
}

fn find_monitor(name: &str) -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    for index in 0..monitors.n_items() {
        let item = monitors.item(index)?;
        let monitor = item.downcast::<gdk::Monitor>().ok()?;
        if let Some(model) = monitor.model() {
            if model == name {
                return Some(monitor);
            }
        }
    }
    None
}
