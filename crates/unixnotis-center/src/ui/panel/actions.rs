//! Panel action row construction

use gtk::prelude::*;
use unixnotis_core::css::hooks;

pub(super) struct PanelActionWidgets {
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

pub(super) fn build_panel_actions() -> PanelActionArea {
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class(hooks::panel_action::ROW);

    // Keep the primary row separate so the close action can stay visually isolated
    let action_primary = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    action_primary.add_css_class(hooks::panel_action::GROUP);

    let focus_toggle = build_text_toggle_action(
        hooks::panel_action::FOCUS,
        "applications-system-symbolic",
        "Widgets",
        "Toggle widget visibility",
    );

    let dnd_toggle = build_text_toggle_action(
        hooks::panel_action::PRIMARY,
        "weather-clear-night-symbolic",
        "DND",
        "Silence incoming notifications",
    );

    let clear_button = build_text_button_action(
        hooks::panel_action::MUTED,
        "user-trash-symbolic",
        "Clear",
        "Clear all notifications",
    );

    let search_toggle = build_icon_toggle_action(
        hooks::panel_action::SEARCH,
        "system-search-symbolic",
        "Toggle search",
    );

    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.add_css_class(hooks::panel_action::ROOT);
    close_button.add_css_class(hooks::panel_action::ICON_ONLY);
    close_button.add_css_class(hooks::panel_action::CLOSE);
    close_button.set_tooltip_text(Some("Close panel"));

    // Action order stays fixed so themes can rely on the row structure
    action_primary.append(&focus_toggle);
    action_primary.append(&dnd_toggle);
    action_primary.append(&clear_button);
    action_primary.append(&search_toggle);
    actions.append(&action_primary);

    PanelActionArea {
        row: actions,
        widgets: PanelActionWidgets {
            focus_toggle,
            dnd_toggle,
            clear_button,
            search_toggle,
            close_button,
        },
    }
}

fn build_text_toggle_action(
    role_class: &str,
    icon_name: &str,
    label_text: &str,
    tooltip: &str,
) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    // Shared button setup keeps text and icon actions visually aligned
    configure_action_button(&button, role_class, false);
    button.set_tooltip_text(Some(tooltip));

    let content = build_labeled_action_content(icon_name, label_text);
    button.set_child(Some(&content));
    button
}

fn build_text_button_action(
    role_class: &str,
    icon_name: &str,
    label_text: &str,
    tooltip: &str,
) -> gtk::Button {
    let button = gtk::Button::new();
    // Plain buttons reuse the same shell so role classes stay the only visual difference
    configure_action_button(&button, role_class, false);
    button.set_tooltip_text(Some(tooltip));

    let content = build_labeled_action_content(icon_name, label_text);
    button.set_child(Some(&content));
    button
}

fn build_icon_toggle_action(role_class: &str, icon_name: &str, tooltip: &str) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    // Icon-only actions still use the shared role hooks for theme overrides
    configure_action_button(&button, role_class, true);
    button.set_tooltip_text(Some(tooltip));

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.add_css_class(hooks::panel_action::GLYPH);
    button.set_child(Some(&icon));
    button
}

fn build_labeled_action_content(icon_name: &str, label_text: &str) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.add_css_class(hooks::panel_action::CONTENT);

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.add_css_class(hooks::panel_action::GLYPH);

    let label = gtk::Label::new(Some(label_text));
    label.add_css_class(hooks::panel_action::LABEL);

    // One small content box keeps icon spacing and text alignment consistent
    content.append(&icon);
    content.append(&label);
    content
}

fn configure_action_button(button: &impl IsA<gtk::Widget>, role_class: &str, icon_only: bool) {
    // Base class keeps the shared shell while role hooks handle per-action styling
    button.add_css_class(hooks::panel_action::ROOT);
    button.add_css_class(role_class);
    if icon_only {
        button.add_css_class(hooks::panel_action::ICON_ONLY);
    } else {
        button.add_css_class(hooks::panel_action::WITH_ICON);
    }
}
