//! Kind-specific card styling helpers

use gtk::prelude::*;
use unixnotis_core::{css::hooks, CardWidgetConfig};

pub(super) fn apply_card_kind_classes(root: &gtk::Box, config: &CardWidgetConfig) {
    if config.monospace {
        // Monospace stays as a separate hook so command cards can opt in without new kinds
        root.add_css_class(hooks::info_card::MONO);
    }
    if let Some(kind) = config.kind.as_deref() {
        if let Some(class) =
            super::super::kind_css::widget_kind_css_class("unixnotis-info-card-kind-", kind)
        {
            // User-defined card kinds get stable hooks without changing built-in classes
            root.add_css_class(&class);
        }
        match kind {
            "calendar" => root.add_css_class(hooks::info_card::CALENDAR),
            "weather" => root.add_css_class(hooks::info_card::WEATHER),
            _ => {}
        }
    }
}

pub(super) fn configure_card_icon(icon: &gtk::Image, config: &CardWidgetConfig) {
    if matches!(config.kind.as_deref(), Some("weather")) {
        // Weather icons need a larger slot so symbolic weather sets do not look cramped
        icon.set_pixel_size(24);
        icon.add_css_class("unixnotis-info-icon-weather");
    } else {
        icon.set_pixel_size(18);
    }
    icon.add_css_class(hooks::info_card::ICON);
}

pub(super) fn card_icon_size(config: &CardWidgetConfig) -> u32 {
    // Asset decoding and theme icons share one fixed logical slot per card kind
    if matches!(config.kind.as_deref(), Some("weather")) {
        24
    } else {
        18
    }
}
