//! Notification row widget construction
//!
//! This file builds the reusable GTK widgets once and leaves refresh logic elsewhere

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::pango::{EllipsizeMode, WrapMode};
use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::css::hooks;

use crate::dbus::UiCommand;
use crate::ui::try_send_command;

use super::state::NotificationRowWidgets;

pub(in crate::ui::list) fn build_notification_row(
    command_tx: mpsc::Sender<UiCommand>,
) -> (gtk::Box, NotificationRowWidgets) {
    // Root owns the full collapsed-stack shape as one ListView row
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class(hooks::panel_card::ROW);
    root.set_hexpand(true);

    // Card uses vertical layout: header, summary, body, then actions
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("unixnotis-panel-card");
    card.set_hexpand(true);

    // Header packs icon + app label + close button
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let icon = gtk::Image::new();
    icon.set_pixel_size(22);
    icon.add_css_class("unixnotis-panel-icon");

    let app_label = gtk::Label::new(None);
    app_label.set_xalign(0.0);
    // Ellipsis avoids row width spikes from long app names
    app_label.set_ellipsize(EllipsizeMode::End);
    app_label.set_single_line_mode(true);
    app_label.set_max_width_chars(40);
    app_label.add_css_class("unixnotis-panel-app");

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    // Spacer pushes close button to the far edge
    spacer.set_hexpand(true);

    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.set_halign(gtk::Align::End);
    close_button.add_css_class("unixnotis-panel-close");

    header.append(&icon);
    header.append(&app_label);
    header.append(&spacer);
    header.append(&close_button);

    // Summary is optional, so the update path decides later if the row should exist
    let summary_label = gtk::Label::new(None);
    summary_label.set_xalign(0.0);
    // Summary can wrap but stays bounded to three lines
    summary_label.set_wrap(true);
    summary_label.set_wrap_mode(WrapMode::WordChar);
    summary_label.set_ellipsize(EllipsizeMode::End);
    summary_label.set_lines(3);
    summary_label.set_max_width_chars(88);
    summary_label.add_css_class("unixnotis-panel-summary");

    // Body follows the same optional-row rule as summary text
    let body_label = gtk::Label::new(None);
    body_label.set_xalign(0.0);
    // Body gets more lines than summary but still has upper bounds
    body_label.set_wrap(true);
    body_label.set_wrap_mode(WrapMode::WordChar);
    body_label.set_ellipsize(EllipsizeMode::End);
    body_label.set_lines(8);
    body_label.set_max_width_chars(112);
    body_label.add_css_class("unixnotis-panel-body");

    let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    // Action buttons are added on demand during row updates
    actions_box.add_css_class("unixnotis-notification-actions");

    // Keep the card tree fully built up front
    // Row refreshes then only replace content instead of rebuilding containers
    card.append(&header);
    card.append(&summary_label);
    card.append(&body_label);
    card.append(&actions_box);

    let stack_ghost_1 = build_stack_ghost(1);
    let stack_ghost_2 = build_stack_ghost(2);

    // Ghost cards are part of the same row so stack depth updates in one bind pass
    root.append(&card);
    root.append(&stack_ghost_1);
    root.append(&stack_ghost_2);

    let notify_id = Rc::new(Cell::new(0));
    // Close click always targets the latest id assigned to this row
    let close_tx = command_tx.clone();
    let notify_id_clone = notify_id.clone();
    close_button.connect_clicked(move |_| {
        let id = notify_id_clone.get();
        if id == 0 {
            // Ignore clicks before first binding
            return;
        }
        debug!(id, "dismiss clicked");
        // Non-blocking enqueue avoids GTK stalls during D-Bus backpressure
        try_send_command(&close_tx, UiCommand::Dismiss(id));
    });

    // The reusable widget bundle is returned with the root so the list factory
    // can keep the GTK tree and the cached row state together
    (
        root,
        NotificationRowWidgets {
            card,
            stack_ghost_1,
            stack_ghost_2,
            icon,
            app_label,
            summary_label,
            body_label,
            actions_box,
            notify_id,
            action_cache: RefCell::new(Vec::new()),
            icon_sig: RefCell::new(None),
        },
    )
}

fn build_stack_ghost(depth: u8) -> gtk::Box {
    let ghost = gtk::Box::new(gtk::Orientation::Vertical, 0);
    // The real card and its shadows share theme hooks for consistent colors
    ghost.add_css_class("unixnotis-panel-card");
    ghost.add_css_class("unixnotis-stack-ghost");
    ghost.add_css_class(&format!("unixnotis-stack-ghost-{depth}"));
    ghost.set_hexpand(true);
    ghost.set_visible(false);
    ghost
}
