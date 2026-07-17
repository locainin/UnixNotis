use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Overflow, PolicyType};
use unixnotis_core::{hooks, MediaConfig};

use crate::media::MediaHandle;

use super::super::card::MediaCardWidgets;
use super::super::layout::{
    card_height_for_shell, card_layout_class, marquee_width_for_shell,
    marquee_width_for_shell_player_count, media_content_width, row_layout_class,
    stack_layout_class,
};
use super::super::parts::build_media_card_parts;
use super::super::selection::MediaSelection;
use super::super::shell::{apply_shell_state_classes, MediaShellConfig};
use super::plan::ShellCompositionPlan;
use super::shell::compose_card_shell;

pub(in super::super) struct MediaWidgetParts {
    pub(in super::super) root: gtk::Box,
    pub(in super::super) nav_prev: gtk::Button,
    pub(in super::super) nav_next: gtk::Button,
    pub(in super::super) card: MediaCardWidgets,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MediaWidthBoundaryConfig {
    // Exact content budget inherited from the configured panel width
    pub(super) content_width: i32,
    // External policy clips oversized children without drawing a scrollbar
    pub(super) horizontal_policy: PolicyType,
    // Media rows never need a vertical scrollbar inside the panel
    pub(super) vertical_policy: PolicyType,
    // Disabling propagation stops child natural width from resizing the panel
    pub(super) propagate_natural_width: bool,
}

pub(super) const fn media_width_boundary_config(content_width: i32) -> MediaWidthBoundaryConfig {
    MediaWidthBoundaryConfig {
        content_width,
        horizontal_policy: PolicyType::External,
        vertical_policy: PolicyType::Never,
        propagate_natural_width: false,
    }
}

pub(in super::super) fn build_media_widget(
    handle: &MediaHandle,
    selection: Rc<RefCell<MediaSelection>>,
    panel_width: i32,
    config: &MediaConfig,
    shell: &MediaShellConfig,
) -> MediaWidgetParts {
    // The plan freezes shell routing before any widgets are created
    let plan = ShellCompositionPlan::from_shell(shell);
    // Content width is shared by the outer stack and the row inside it
    let content_width = media_content_width(panel_width);
    let root = gtk::Box::new(
        gtk::Orientation::Vertical,
        shell.navigation_spacing_px.max(shell.content_spacing_px),
    );
    root.add_css_class(hooks::media_shell::STACK);
    root.add_css_class(stack_layout_class(shell.layout));
    root.set_size_request(content_width, -1);
    root.set_hexpand(true);
    root.set_halign(Align::Fill);
    root.set_overflow(Overflow::Hidden);
    root.set_visible(false);
    // Shell state classes let themes react to hidden art or control placement without guessing
    apply_shell_state_classes(&root, shell);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, shell.navigation_spacing_px);
    row.add_css_class(hooks::media_shell::ROW);
    row.add_css_class(row_layout_class(shell.layout));
    row.set_size_request(content_width, -1);
    row.set_hexpand(true);
    row.set_halign(Align::Fill);
    row.set_valign(Align::Center);
    row.set_overflow(Overflow::Hidden);
    apply_shell_state_classes(&row, shell);

    // Outer nav buttons are reused by carousel and in-card navigation shells
    let nav_prev = build_navigation_button("<", hooks::media_shell::NAV_PREV);
    let nav_next = build_navigation_button(">", hooks::media_shell::NAV_NEXT);
    // The marquee budget depends on the final shell, not just the panel width
    let marquee_width = marquee_width_for_shell(shell, panel_width);
    let parts = build_media_card_parts(handle, selection, marquee_width, config, shell);
    parts
        .card
        .single_player_text_width
        .set(marquee_width_for_shell_player_count(
            shell,
            panel_width,
            false,
        ));

    parts
        .card
        .root
        .add_css_class(card_layout_class(shell.layout));
    // Card height still stays explicit so the panel does not jitter while metadata changes
    parts
        .card
        .root
        .set_size_request(-1, card_height_for_shell(shell));
    parts.card.apply_display_config(config);
    apply_shell_state_classes(&parts.card.root, shell);

    // Shell composition mounts art, text, controls, and nav into the final card
    compose_card_shell(shell, &plan, &parts, &nav_prev, &nav_next);

    if plan.external_nav {
        // Carousel-style navigation keeps player switching outside the card shell
        row.append(&nav_prev);
        row.append(&parts.card.root);
        row.append(&nav_next);
    } else {
        row.append(&parts.card.root);
    }

    // A scrolled boundary prevents a wide theme or long media child from increasing panel width
    // Scrollbars stay disabled because the media budget already fits supported stock layouts
    let width_boundary = build_media_width_boundary(&row, content_width);

    root.append(&width_boundary);
    MediaWidgetParts {
        root,
        nav_prev,
        nav_next,
        card: parts.card,
    }
}

pub(super) fn build_media_width_boundary(
    row: &gtk::Box,
    content_width: i32,
) -> gtk::ScrolledWindow {
    let config = media_width_boundary_config(content_width);
    let width_boundary = gtk::ScrolledWindow::new();
    // External horizontal policy allows clipping without a visible scrollbar or child minimum leak
    width_boundary.set_policy(config.horizontal_policy, config.vertical_policy);
    width_boundary.set_propagate_natural_width(config.propagate_natural_width);
    super::super::card::set_scrolled_content_width(&width_boundary, config.content_width);
    width_boundary.set_hexpand(true);
    width_boundary.set_halign(Align::Fill);
    width_boundary.set_overflow(Overflow::Hidden);
    width_boundary.set_child(Some(row));
    width_boundary
}

pub(super) fn build_navigation_button(label_text: &str, role_class: &str) -> gtk::Button {
    // These buttons are text-backed on purpose so themes can restyle the glyphs freely
    let button = gtk::Button::with_label(label_text);
    button.add_css_class(hooks::media_shell::NAV);
    button.add_css_class(role_class);
    // Explicit centering keeps tiny glyph buttons aligned across themes
    button.set_halign(Align::Center);
    button.set_valign(Align::Center);
    if let Some(label) = button
        .child()
        .and_then(|child| child.downcast::<gtk::Label>().ok())
    {
        // Centering the child label avoids off-by-one drift between themes and fonts
        label.set_xalign(0.5);
        label.set_yalign(0.5);
    }
    button
}
