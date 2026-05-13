//! Widget construction helpers for the panel UI.
//!
//! Keeps layout assembly for quick controls and extra widgets centralized so
//! config reloads can rebuild sections consistently.

use gtk::prelude::*;

use unixnotis_core::{css::hooks, Config};

use super::{panel, widgets};

pub(super) fn build_quick_controls(
    panel: &panel::PanelWidgets,
    config: &Config,
) -> (
    Option<widgets::volume::VolumeWidget>,
    Option<widgets::brightness::BrightnessWidget>,
) {
    // Quick controls are compact widgets above the notification list.
    let mut has_widgets = false;
    let volume = if config.widgets.volume.enabled {
        let widget = widgets::volume::VolumeWidget::new(config.widgets.volume.clone());
        panel.quick_controls.append(widget.root());
        has_widgets = true;
        Some(widget)
    } else {
        None
    };

    let brightness = if config.widgets.brightness.enabled {
        let widget = widgets::brightness::BrightnessWidget::new(config.widgets.brightness.clone());
        panel.quick_controls.append(widget.root());
        has_widgets = true;
        Some(widget)
    } else {
        None
    };

    panel.quick_controls.set_visible(has_widgets);
    (volume, brightness)
}

pub(super) fn build_extra_widgets(
    panel: &panel::PanelWidgets,
    config: &Config,
) -> (
    Option<widgets::toggles::ToggleGrid>,
    Option<widgets::stats::StatGrid>,
    Option<widgets::cards::CardGrid>,
) {
    // Toggle widgets represent binary state controls and their watchers.
    let toggles = widgets::toggles::ToggleGrid::new(
        &config.widgets.toggles,
        config.widgets.toggle_tooltips,
        config.widgets.toggle_layout,
        config.widgets.toggle_columns,
    );
    if let Some(grid) = toggles.as_ref() {
        panel.toggle_container.set_visible(true);
        panel.toggle_container.append(grid.root());
    } else {
        panel.toggle_container.set_visible(false);
    }

    // Stats widgets expose periodic metrics like CPU and memory usage.
    let stats = widgets::stats::StatGrid::new(&config.widgets.stats, config.widgets.stat_columns);
    if let Some(grid) = stats.as_ref() {
        panel.stat_container.set_visible(true);
        panel.stat_container.append(grid.root());
    } else {
        panel.stat_container.set_visible(false);
    }

    // Card widgets are larger, multi-line information tiles.
    let cards = widgets::cards::CardGrid::new(&config.widgets.cards, config.widgets.card_columns);
    if let Some(grid) = cards.as_ref() {
        panel.card_container.set_visible(true);
        panel.card_container.append(grid.root());
    } else {
        panel.card_container.set_visible(false);
    }

    (toggles, stats, cards)
}

pub(super) fn clear_container(container: &gtk::Box) {
    // Clear children before repopulating to avoid duplicates on reload.
    while let Some(child) = container.last_child() {
        if child.has_css_class(hooks::panel_shell::SECTION_HEADER) {
            break;
        }
        container.remove(&child);
    }
}
