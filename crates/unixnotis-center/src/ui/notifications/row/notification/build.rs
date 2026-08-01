//! Notification row widget construction
//!
//! This file builds the reusable GTK widgets once and leaves refresh logic elsewhere

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::pango::{EllipsizeMode, WrapMode};
use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::{css::hooks, NotificationKey};
use unixnotis_ui::presentation::default_activation::{
    connect_default_activation, mark_interactive,
};
use unixnotis_ui::CutCorner;

use crate::control::UiCommand;
use crate::ui::try_send_command;

use super::reply::build_inline_reply;
use super::stack::append_stack_layers;
use super::state::NotificationRowWidgets;

pub(in crate::ui::notifications) fn build_notification_row(
    command_tx: mpsc::Sender<UiCommand>,
) -> (gtk::Box, NotificationRowWidgets) {
    // Root owns the full collapsed group preview as one ListView row
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class(hooks::panel_card::ROW);
    root.set_hexpand(true);

    // Card uses vertical layout: header, summary, body, then actions
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("unixnotis-panel-card");
    card.set_hexpand(true);

    let meta_top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    meta_top.add_css_class(hooks::panel_card::META_TOP);
    meta_top.set_hexpand(true);
    meta_top.set_visible(false);

    let meta_label = gtk::Label::new(None);
    meta_label.add_css_class(hooks::panel_card::META_LABEL);
    meta_label.set_xalign(0.0);
    meta_label.set_single_line_mode(true);

    let time_badge = gtk::Label::new(None);
    time_badge.add_css_class(hooks::panel_card::TIME_BADGE);
    time_badge.set_halign(gtk::Align::End);
    time_badge.set_xalign(1.0);
    time_badge.set_single_line_mode(true);
    time_badge.set_visible(false);
    meta_top.append(&meta_label);

    // The dismiss control stays in the measured header like the stable master layout
    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.set_halign(gtk::Align::End);
    close_button.set_valign(gtk::Align::Center);
    close_button.add_css_class("unixnotis-panel-close");
    mark_interactive(&close_button);
    close_button.update_property(&[gtk::accessible::Property::Label("Dismiss notification")]);

    // Header owns identity, chronology, and dismiss without covering message content
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.add_css_class(hooks::panel_card::HEADER);
    let icon = gtk::Image::new();
    icon.set_pixel_size(20);
    icon.add_css_class("unixnotis-panel-icon");

    let identity = gtk::Box::new(gtk::Orientation::Vertical, 1);
    identity.set_hexpand(true);
    let identity_top = gtk::Box::new(gtk::Orientation::Horizontal, 6);

    let app_label = gtk::Label::new(None);
    app_label.set_xalign(0.0);
    // Ellipsis avoids row width spikes from long app names
    app_label.set_ellipsize(EllipsizeMode::End);
    app_label.set_single_line_mode(true);
    app_label.set_max_width_chars(40);
    app_label.add_css_class("unixnotis-panel-app");

    let trust_chip = gtk::Label::new(None);
    trust_chip.set_single_line_mode(true);
    trust_chip.add_css_class("unixnotis-panel-trust-chip");
    trust_chip.set_visible(false);

    let secondary_claim = gtk::Label::new(None);
    secondary_claim.set_xalign(0.0);
    secondary_claim.set_single_line_mode(true);
    secondary_claim.set_ellipsize(EllipsizeMode::End);
    secondary_claim.add_css_class("unixnotis-panel-secondary-claim");
    secondary_claim.set_visible(false);

    let urgency_badge = gtk::Label::new(Some("Critical"));
    // Reused rows toggle this widget instead of rebuilding the header tree
    urgency_badge.add_css_class(hooks::urgency::BADGE);
    urgency_badge.set_single_line_mode(true);
    urgency_badge.set_visible(false);

    identity_top.append(&app_label);
    identity_top.append(&trust_chip);
    identity_top.append(&urgency_badge);
    identity.append(&identity_top);
    identity.append(&secondary_claim);
    header.append(&icon);
    header.append(&identity);
    header.append(&time_badge);
    header.append(&close_button);

    let body_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    body_row.set_hexpand(true);

    let thumbnail = gtk::Image::new();
    thumbnail.add_css_class(hooks::panel_card::THUMBNAIL);
    thumbnail.set_pixel_size(56);
    thumbnail.set_size_request(56, 56);
    thumbnail.set_visible(false);

    let text_stack = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text_stack.add_css_class(hooks::panel_card::TEXT);
    text_stack.set_hexpand(true);

    // Summary is optional, so the update path decides later if the row should exist
    let summary_label = gtk::Label::new(None);
    summary_label.set_xalign(0.0);
    summary_label.set_hexpand(true);
    // One title line keeps short grouped rows compact
    summary_label.set_wrap(true);
    summary_label.set_wrap_mode(WrapMode::WordChar);
    summary_label.set_ellipsize(EllipsizeMode::End);
    summary_label.set_lines(1);
    summary_label.set_max_width_chars(88);
    summary_label.add_css_class("unixnotis-panel-summary");

    // Body follows the same optional-row rule as summary text
    let body_label = gtk::Label::new(None);
    body_label.set_xalign(0.0);
    // Three body lines provide context without dominating the panel
    body_label.set_wrap(true);
    body_label.set_wrap_mode(WrapMode::WordChar);
    body_label.set_ellipsize(EllipsizeMode::End);
    body_label.set_lines(3);
    body_label.set_max_width_chars(112);
    body_label.add_css_class("unixnotis-panel-body");

    let popup_status = gtk::Label::new(None);
    popup_status.set_xalign(0.0);
    popup_status.set_wrap(true);
    popup_status.set_wrap_mode(WrapMode::WordChar);
    popup_status.set_lines(2);
    popup_status.add_css_class("unixnotis-popup-status");
    popup_status.set_visible(false);

    text_stack.append(&summary_label);
    text_stack.append(&body_label);
    text_stack.append(&popup_status);
    body_row.append(&thumbnail);
    body_row.append(&text_stack);

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    footer.add_css_class(hooks::panel_card::FOOTER);
    footer.set_hexpand(true);
    footer.set_visible(false);

    let footer_left = gtk::Label::new(None);
    footer_left.add_css_class(hooks::panel_card::FOOTER_LEFT);
    footer_left.set_xalign(0.0);
    footer_left.set_single_line_mode(true);

    let footer_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    footer_spacer.set_hexpand(true);

    let footer_right = gtk::Label::new(None);
    footer_right.add_css_class(hooks::panel_card::FOOTER_RIGHT);
    footer_right.set_xalign(1.0);
    footer_right.set_single_line_mode(true);
    footer.append(&footer_left);
    footer.append(&footer_spacer);
    footer.append(&footer_right);

    let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    // Action buttons are added on demand during row updates
    actions_box.add_css_class("unixnotis-notification-actions");
    mark_interactive(&actions_box);
    let inline_reply = build_inline_reply(command_tx.clone());

    // Keep the card tree fully built up front
    // Row refreshes then only replace content instead of rebuilding containers
    card.append(&meta_top);
    card.append(&header);
    card.append(&body_row);
    card.append(&footer);
    card.append(&actions_box);
    card.append(&inline_reply.revealer);

    // The wrapper clips the complete styled card while the inner box keeps all CSS hooks
    let card_plate = CutCorner::new(&card, unixnotis_core::CutCorners::default());
    card_plate.add_css_class("unixnotis-panel-card-foreground");

    // Rear layers and the readable foreground remain one virtualized ListView row
    let (stack_middle, stack_back) = append_stack_layers(&root, &card_plate);

    let notify_key = Rc::new(Cell::new(NotificationKey {
        id: 0,
        generation: 0,
    }));
    // Recycled rows retain the exact generation rather than targeting a reused numeric id
    connect_dismiss_button(&close_button, command_tx.clone(), notify_key.clone());
    let default_activation = connect_default_activation(&card, {
        move |notification, action_key| {
            try_send_command(
                &command_tx,
                UiCommand::InvokeAction {
                    notification,
                    action_key,
                    confirmed: false,
                },
            );
        }
    });

    // The reusable widget bundle is returned with the root so the list factory
    // can keep the GTK tree and the cached row state together
    (
        root,
        NotificationRowWidgets {
            default_activation,
            card,
            card_plate,
            stack_middle,
            stack_back,
            icon,
            header,
            app_label,
            secondary_claim,
            trust_chip,
            urgency_badge,
            close_button,
            meta_top,
            meta_label,
            time_badge,
            thumbnail,
            summary_label,
            body_label,
            popup_status,
            footer,
            footer_left,
            footer_right,
            actions_box,
            inline_reply,
            notify_key,
            action_cache_key: Cell::new(NotificationKey {
                id: 0,
                generation: 0,
            }),
            action_cache: RefCell::new(Vec::new()),
            reply_cache: RefCell::new((
                unixnotis_core::InlineReply::default(),
                unixnotis_core::InlineReplyPolicy::Deny,
                false,
            )),
            icon_sig: RefCell::new(None),
        },
    )
}

fn connect_dismiss_button(
    button: &gtk::Button,
    command_tx: mpsc::Sender<UiCommand>,
    notify_key: Rc<Cell<NotificationKey>>,
) {
    button.connect_clicked(move |_| {
        let notification = notify_key.get();
        if notification.id == 0 {
            // Ignore clicks before first binding
            return;
        }
        debug!(
            id = notification.id,
            generation = notification.generation,
            "dismiss clicked"
        );
        // Non-blocking enqueue avoids GTK stalls during D-Bus backpressure
        try_send_command(&command_tx, UiCommand::Dismiss(notification));
    });
}
