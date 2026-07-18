//! Command slider GTK construction

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{css::hooks, SliderWidgetConfig};

use super::icons::resolve_slider_icon_name;
use super::layout::build_slider_stack;

pub(in super::super) struct SliderWidgets {
    // Root row used by volume, brightness, and custom slider wrappers
    pub(in super::super) root: gtk::Box,
    // Range control exposed back to refresh and action wiring
    pub(in super::super) scale: gtk::Scale,
    // Optional numeric value shown beside the slider
    pub(in super::super) value_label: gtk::Label,
    // Icon image is shared by clickable and non-clickable icon shells
    pub(in super::super) icon_image: gtk::Image,
    // Resolved normal icon name after theme fallback handling
    pub(in super::super) icon_name: String,
    // Resolved muted icon name when the config provides one
    pub(in super::super) icon_muted: Option<String>,
}

pub(in super::super) fn build_slider_widgets(
    config: &SliderWidgetConfig,
    extra_class: &str,
) -> SliderWidgets {
    // Root combines base style with caller-provided variant class
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    root.add_css_class(hooks::slider::ROOT);
    root.add_css_class(extra_class);

    // Resolve icon upfront so themes without exact names still render valid glyphs
    let icon_name = resolve_slider_icon_name(&config.label, &config.icon);
    let icon_image = gtk::Image::from_icon_name(&icon_name);
    icon_image.set_valign(Align::Center);
    icon_image.set_halign(Align::Center);

    let scale = gtk::Scale::with_range(
        gtk::Orientation::Horizontal,
        config.min,
        config.max,
        config.step,
    );
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.set_vexpand(false);
    scale.set_valign(Align::Center);
    // One size request is enough here and keeps the layout less rigid
    scale.set_size_request(180, 24);
    scale.add_css_class(hooks::slider::SCALE);

    // Keep the first frame neutral so generic sliders do not imply a percent value
    let value_label = gtk::Label::new(Some("----"));
    value_label.add_css_class(hooks::slider::VALUE);
    value_label.set_valign(Align::Center);
    value_label.set_xalign(1.0);
    value_label.set_width_chars(4);
    value_label.set_visible(config.show_value);

    let slider_stack = build_slider_stack(&scale, config);
    root.append(&slider_stack);
    root.append(&value_label);

    let icon_muted = config
        .icon_muted
        .as_deref()
        .map(|name| resolve_slider_icon_name(&config.label, name));

    SliderWidgets {
        root,
        scale,
        value_label,
        icon_image,
        icon_name,
        icon_muted,
    }
}

#[cfg(test)]
#[path = "tests/build.rs"]
mod tests;
