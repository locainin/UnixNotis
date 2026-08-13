use std::collections::HashSet;

use super::{MAX_WIDGET_COLUMNS, MIN_WIDGET_COLUMNS};
use crate::{
    default_panel_action_order, default_panel_section_order, default_panel_widget_order, Config,
    DndMenuChoice, NotificationMetadataConfig, PanelActionConfig, PanelActionId, PanelConfig,
    PanelSection, PanelWidgetSection,
};

const MAX_DND_MENU_CHOICES: usize = 16;
const MAX_DND_DURATION_MINUTES: u32 = 525_600;
const MAX_DND_LABEL_CHARS: usize = 96;
const MAX_METADATA_TEXT_CHARS: usize = 128;
const MAX_ICON_NAME_CHARS: usize = 128;

pub(super) fn sanitize_panel_text(panel: &mut PanelConfig) {
    // Empty core labels make the panel harder to operate, so restore only required text
    if panel.title.trim().is_empty() {
        panel.title = PanelConfig::default().title;
    }
    if panel.clear_label.trim().is_empty() {
        panel.clear_label = PanelConfig::default().clear_label;
    }
    if panel.search_magnifier_icon.trim().is_empty() {
        panel.search_magnifier_icon = PanelConfig::default().search_magnifier_icon;
    }
    truncate_chars(&mut panel.search_magnifier_icon, MAX_ICON_NAME_CHARS);
    sanitize_action_config(&mut panel.focus_action, PanelActionConfig::widgets());
    sanitize_action_config(&mut panel.dnd_action, PanelActionConfig::dnd());
    sanitize_action_config(&mut panel.clear_action, PanelActionConfig::clear());
    sanitize_action_config(&mut panel.search_action, PanelActionConfig::search());
    sanitize_action_config(&mut panel.close_action, PanelActionConfig::close());
}

pub(super) fn sanitize_panel_section_order(order: &mut Vec<PanelSection>) {
    sanitize_order(order, default_panel_section_order);
}

pub(super) fn sanitize_panel_widget_order(order: &mut Vec<PanelWidgetSection>) {
    sanitize_order(order, default_panel_widget_order);
}

pub(super) fn sanitize_panel_action_order(order: &mut Vec<PanelActionId>) {
    sanitize_order(order, default_panel_action_order);
}

pub(super) fn sanitize_dnd_menu(panel: &mut PanelConfig) {
    let mut seen = HashSet::new();
    // An empty trigger list deliberately disables the context menu
    panel
        .dnd_menu_triggers
        .retain(|trigger| seen.insert(*trigger));
    panel.dnd_menu_choices.truncate(MAX_DND_MENU_CHOICES);
    panel.dnd_menu_choices.retain_mut(|choice| {
        let label = choice.label_mut();
        truncate_chars(label, MAX_DND_LABEL_CHARS);
        if label.trim().is_empty() {
            // Empty buttons are unusable and should not occupy menu space
            return false;
        }

        match choice {
            DndMenuChoice::Duration { minutes, .. } => {
                *minutes = (*minutes).clamp(1, MAX_DND_DURATION_MINUTES);
            }
            DndMenuChoice::Tomorrow { hour, minute, .. } => {
                *hour = (*hour).min(23);
                *minute = (*minute).min(59);
            }
            DndMenuChoice::Indefinite { .. } => {}
        }
        true
    });
}

pub(super) fn sanitize_notification_metadata(config: &mut NotificationMetadataConfig) {
    for text in [
        &mut config.critical_label,
        &mut config.low_label,
        &mut config.normal_label,
        &mut config.relative_now,
        &mut config.relative_minutes,
        &mut config.relative_hours,
        &mut config.relative_days,
        &mut config.transient_label,
        &mut config.live_label,
        &mut config.history_label,
        &mut config.action_count_one,
        &mut config.action_count_many,
    ] {
        // Empty metadata text is valid because it hides that optional badge
        truncate_chars(text, MAX_METADATA_TEXT_CHARS);
    }
}

fn sanitize_order<T>(order: &mut Vec<T>, defaults: fn() -> Vec<T>)
where
    T: Copy + Eq + std::hash::Hash,
{
    let default_order = defaults();
    if order.is_empty() {
        // An empty order means "use the stock order", not "render no sections"
        *order = default_order;
        return;
    }
    let mut seen = HashSet::new();
    // Keep the first occurrence so user intent survives when duplicates are present
    order.retain(|section| seen.insert(*section));
    for section in default_order {
        if !seen.contains(&section) {
            // Missing sections are appended so future defaults remain reachable after upgrades
            order.push(section);
        }
    }
}

fn sanitize_action_config(config: &mut PanelActionConfig, default: PanelActionConfig) {
    if config.label.trim().is_empty()
        && config.icon.trim().is_empty()
        && config.tooltip.trim().is_empty()
        && !config.icon_only
    {
        // Empty blocks from partial config files mean "use stock" when no mode was set
        *config = default;
        return;
    }
    if config.label.trim().is_empty() && !config.icon_only {
        // Empty labels remain possible by using icon_only=true explicitly
        config.label = default.label;
    }
    if config.icon.trim().is_empty() {
        config.icon = default.icon;
    }
    if config.tooltip.trim().is_empty() {
        config.tooltip = default.tooltip;
    }
}

pub(super) fn sanitize_widget_columns(config: &mut Config) {
    let defaults = crate::WidgetsConfig::default();
    config.widgets.toggle_columns =
        sanitize_column_count(config.widgets.toggle_columns, defaults.toggle_columns);
    config.widgets.stat_columns =
        sanitize_column_count(config.widgets.stat_columns, defaults.stat_columns);
    config.widgets.card_columns =
        sanitize_column_count(config.widgets.card_columns, defaults.card_columns);
}

fn sanitize_column_count(value: usize, default_value: usize) -> usize {
    if value == 0 {
        // Zero is treated as "auto/default" because it cannot form a usable grid
        return default_value;
    }
    value.clamp(MIN_WIDGET_COLUMNS, MAX_WIDGET_COLUMNS)
}

fn truncate_chars(value: &mut String, max_chars: usize) {
    // UTF-8 boundaries are found through char indices before truncation
    let Some((index, _)) = value.char_indices().nth(max_chars) else {
        return;
    };
    value.truncate(index);
}
