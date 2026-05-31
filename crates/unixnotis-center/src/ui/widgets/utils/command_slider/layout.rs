//! Slider GTK layout helpers

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{css::hooks, SliderWidgetConfig};

const MAX_RENDERED_SEGMENTS: usize = 64;
const MAX_SUBLABEL_CHARS: usize = 32;

pub(super) fn build_icon_shell(icon_image: &gtk::Image, interactive: bool) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(icon_image));
    button.add_css_class(hooks::slider::ICON);
    button.set_valign(Align::Center);
    button.set_halign(Align::Center);
    // Static shells keep the same GTK node as clickable shells so rows stay aligned
    button.set_focusable(interactive);
    button.set_can_target(interactive);
    button
}

pub(super) fn build_slider_stack(scale: &gtk::Scale, config: &SliderWidgetConfig) -> gtk::Box {
    let stack = gtk::Box::new(gtk::Orientation::Vertical, 2);
    stack.add_css_class(hooks::slider::STACK);
    stack.set_hexpand(true);
    stack.append(scale);

    if config.segments > 0 {
        stack.append(&build_segment_row(config.segments));
    }
    if config.show_sublabels {
        stack.append(&build_sublabel_row(config));
    }
    stack
}

fn build_segment_row(segments: usize) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    row.add_css_class(hooks::slider::SEGMENTS);
    row.set_hexpand(true);

    for _ in 0..segments.min(MAX_RENDERED_SEGMENTS) {
        let segment = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        segment.add_css_class(hooks::slider::SEGMENT);
        segment.set_hexpand(true);
        row.append(&segment);
    }
    row
}

fn build_sublabel_row(config: &SliderWidgetConfig) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.add_css_class(hooks::slider::SUBLABEL_ROW);
    row.set_hexpand(true);

    let min = slider_sublabel(&config.sublabel_min, config.min);
    let min_label = gtk::Label::new(Some(&min));
    min_label.add_css_class(hooks::slider::SUBLABEL_MIN);
    min_label.set_xalign(0.0);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    spacer.set_hexpand(true);

    let max = slider_sublabel(&config.sublabel_max, config.max);
    let max_label = gtk::Label::new(Some(&max));
    max_label.add_css_class(hooks::slider::SUBLABEL_MAX);
    max_label.set_xalign(1.0);

    row.append(&min_label);
    row.append(&spacer);
    row.append(&max_label);
    row
}

pub(super) fn slider_sublabel(configured: &str, fallback: f64) -> String {
    let configured = configured.trim();
    if configured.is_empty() {
        return super::super::slider_parse::format_value(fallback);
    }
    configured.chars().take(MAX_SUBLABEL_CHARS).collect()
}
