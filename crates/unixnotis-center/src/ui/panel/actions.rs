//! Panel action row construction

use gtk::prelude::*;
use unixnotis_core::{
    css::hooks, PanelActionConfig, PanelActionId, PanelClearButtonPlacement, PanelConfig,
};

pub(super) struct PanelActionWidgets {
    pub(super) group: gtk::Box,
    pub(super) focus_toggle: gtk::ToggleButton,
    pub(super) dnd_toggle: gtk::ToggleButton,
    pub(super) clear_button: gtk::Button,
    pub(super) search_toggle: gtk::ToggleButton,
    pub(super) close_button: gtk::Button,
}

pub(super) struct PanelActionArea {
    pub(super) row: gtk::Box,
    pub(super) widgets: PanelActionWidgets,
}

pub(super) fn build_panel_actions(config: &PanelConfig) -> PanelActionArea {
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class(hooks::panel_action::ROW);

    // Keep the primary row separate so the close action can stay visually isolated
    let action_primary = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    action_primary.add_css_class(hooks::panel_action::GROUP);

    let focus_toggle = build_toggle_action(hooks::panel_action::FOCUS, &config.focus_action);

    let dnd_toggle = build_toggle_action(hooks::panel_action::PRIMARY, &config.dnd_action);

    let clear_button =
        build_button_action(hooks::panel_action::MUTED, &resolved_clear_action(config));
    clear_button.set_visible(matches!(
        config.clear_button_placement,
        PanelClearButtonPlacement::ActionRow
    ));

    let search_toggle = build_toggle_action(hooks::panel_action::SEARCH, &config.search_action);

    let close_button = build_button_action(hooks::panel_action::CLOSE, &config.close_action);
    close_button.add_css_class(hooks::panel_action::ROOT);

    append_ordered_actions(
        &action_primary,
        &focus_toggle,
        &dnd_toggle,
        &clear_button,
        &search_toggle,
        &config.action_order,
    );
    actions.append(&action_primary);

    PanelActionArea {
        row: actions,
        widgets: PanelActionWidgets {
            group: action_primary,
            focus_toggle,
            dnd_toggle,
            clear_button,
            search_toggle,
            close_button,
        },
    }
}

pub(super) fn build_clear_button(config: &PanelConfig) -> gtk::Button {
    build_button_action(hooks::panel_action::MUTED, &resolved_clear_action(config))
}

pub(in crate::ui::panel) fn apply_panel_action_config(
    group: &gtk::Box,
    focus_toggle: &gtk::ToggleButton,
    dnd_toggle: &gtk::ToggleButton,
    clear_button: &gtk::Button,
    search_toggle: &gtk::ToggleButton,
    close_button: &gtk::Button,
    config: &PanelConfig,
) {
    update_action_button(
        focus_toggle,
        hooks::panel_action::FOCUS,
        &config.focus_action,
    );
    update_action_button(dnd_toggle, hooks::panel_action::PRIMARY, &config.dnd_action);
    update_action_button(
        clear_button,
        hooks::panel_action::MUTED,
        &resolved_clear_action(config),
    );
    update_action_button(
        search_toggle,
        hooks::panel_action::SEARCH,
        &config.search_action,
    );
    update_action_button(
        close_button,
        hooks::panel_action::CLOSE,
        &config.close_action,
    );
    append_ordered_actions(
        group,
        focus_toggle,
        dnd_toggle,
        clear_button,
        search_toggle,
        &config.action_order,
    );
}

pub(in crate::ui::panel) fn apply_clear_button_config(button: &gtk::Button, config: &PanelConfig) {
    update_action_button(
        button,
        hooks::panel_action::MUTED,
        &resolved_clear_action(config),
    );
}

fn build_toggle_action(role_class: &str, config: &PanelActionConfig) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    // Shared button setup keeps text and icon actions visually aligned
    configure_action_button(&button, role_class, config.icon_only);
    button.set_tooltip_text(Some(&config.tooltip));

    let content = build_action_content(config);
    button.set_child(Some(&content));
    button
}

fn build_button_action(role_class: &str, config: &PanelActionConfig) -> gtk::Button {
    let button = gtk::Button::new();
    // Plain buttons reuse the same shell so role classes stay the only visual difference
    configure_action_button(&button, role_class, config.icon_only);
    button.set_tooltip_text(Some(&config.tooltip));

    let content = build_action_content(config);
    button.set_child(Some(&content));
    button
}

fn build_action_content(config: &PanelActionConfig) -> gtk::Box {
    let spacing = if config.icon_only { 0 } else { 6 };
    let content = gtk::Box::new(gtk::Orientation::Horizontal, spacing);
    content.add_css_class(hooks::panel_action::CONTENT);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);

    let icon = gtk::Image::from_icon_name(&config.icon);
    icon.add_css_class(hooks::panel_action::GLYPH);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);

    let label = gtk::Label::new(Some(&config.label));
    label.add_css_class(hooks::panel_action::LABEL);
    label.set_visible(!config.icon_only && !config.label.is_empty());
    if config.icon_only || config.label.is_empty() {
        label.add_css_class(hooks::panel_action::LABEL_HIDDEN);
    }

    // One small content box keeps icon spacing and text alignment consistent
    content.append(&icon);
    if !config.icon_only {
        content.append(&label);
    }
    content
}

fn update_action_button(
    button: &impl IsA<gtk::Widget>,
    role_class: &str,
    config: &PanelActionConfig,
) {
    // Replacing the tiny content box is simpler and safer than walking child internals
    configure_action_button(button, role_class, config.icon_only);
    button.set_tooltip_text(Some(&config.tooltip));
    let content = build_action_content(config);
    if let Some(button) = button.dynamic_cast_ref::<gtk::Button>() {
        button.set_child(Some(&content));
    } else if let Some(button) = button.dynamic_cast_ref::<gtk::ToggleButton>() {
        button.set_child(Some(&content));
    }
}

fn configure_action_button(button: &impl IsA<gtk::Widget>, role_class: &str, icon_only: bool) {
    // Base class keeps the shared shell while role hooks handle per-action styling
    button.add_css_class(hooks::panel_action::ROOT);
    button.add_css_class(role_class);
    button.set_halign(gtk::Align::Center);
    button.set_valign(gtk::Align::Center);
    if icon_only {
        button.remove_css_class(hooks::panel_action::WITH_ICON);
        button.add_css_class(hooks::panel_action::ICON_ONLY);
    } else {
        button.remove_css_class(hooks::panel_action::ICON_ONLY);
        button.add_css_class(hooks::panel_action::WITH_ICON);
    }
}

fn append_ordered_actions(
    group: &gtk::Box,
    focus_toggle: &gtk::ToggleButton,
    dnd_toggle: &gtk::ToggleButton,
    clear_button: &gtk::Button,
    search_toggle: &gtk::ToggleButton,
    order: &[PanelActionId],
) {
    let mut previous: Option<gtk::Widget> = None;
    for action in order {
        let child: gtk::Widget = match action {
            PanelActionId::Widgets => focus_toggle.clone().upcast(),
            PanelActionId::Dnd => dnd_toggle.clone().upcast(),
            PanelActionId::Clear => clear_button.clone().upcast(),
            PanelActionId::Search => search_toggle.clone().upcast(),
        };
        if child.parent().is_none() {
            group.append(&child);
        }
        group.reorder_child_after(&child, previous.as_ref());
        previous = Some(child);
    }
}

fn resolved_clear_action(config: &PanelConfig) -> PanelActionConfig {
    let mut action = config.clear_action.clone();
    if action == PanelActionConfig::clear() {
        // Keep the legacy clear_label field as the default source for old configs
        action.label.clone_from(&config.clear_label);
    }
    action
}

#[cfg(test)]
mod tests {
    use unixnotis_core::{PanelActionConfig, PanelConfig};

    use super::resolved_clear_action;

    #[test]
    fn legacy_clear_label_updates_stock_clear_action() {
        let config = PanelConfig {
            clear_label: "Wipe".to_string(),
            ..PanelConfig::default()
        };

        assert_eq!(resolved_clear_action(&config).label, "Wipe");
    }

    #[test]
    fn custom_clear_action_with_clear_label_is_not_rewritten() {
        let mut custom = PanelActionConfig::clear();
        custom.icon = "edit-delete-symbolic".to_string();
        let config = PanelConfig {
            clear_label: "Wipe".to_string(),
            clear_action: custom.clone(),
            ..PanelConfig::default()
        };

        assert_eq!(resolved_clear_action(&config), custom);
    }
}
