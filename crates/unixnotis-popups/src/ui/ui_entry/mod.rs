//! Popup entry construction and UI action wiring
//!
//! Keeps popup row assembly split into focused helpers

mod commands;
mod labels;

#[cfg(test)]
mod tests;

use gtk::pango::{EllipsizeMode, WrapMode};
use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{hooks, NotificationView, Urgency};

use super::UiState;
use crate::dbus::UiCommand;
use commands::try_send_command;
use labels::{
    clamp_label_text, has_visible_text, update_optional_label, POPUP_ACTION_LABEL_MAX_CHARS,
    POPUP_APP_MAX_CHARS, POPUP_BODY_MAX_CHARS, POPUP_SUMMARY_MAX_CHARS,
};

pub(super) struct PopupEntry {
    // Keep the last payload so seed reconcile can detect real content changes
    pub(super) notification: NotificationView,
    // Hidden backlog rows stay lightweight until they enter the visible slice
    pub(super) revealer: Option<gtk::Revealer>,
    pub(super) root: Option<gtk::Box>,
}

impl PopupEntry {
    pub(super) fn queued(notification: NotificationView) -> Self {
        // Backlog rows start as plain data and only grow GTK nodes when they become visible
        Self {
            notification,
            revealer: None,
            root: None,
        }
    }

    pub(super) fn is_materialized(&self) -> bool {
        // Both widgets must exist before stack operations can touch this row safely
        self.revealer.is_some() && self.root.is_some()
    }
}

const MAX_POPUP_ACTIONS: usize = 3;

impl UiState {
    pub(super) fn build_popup_entry(&mut self, notification: &NotificationView) -> PopupEntry {
        // Build the GTK row first so the revealer always wraps a ready child
        let root = self.build_popup_root(notification);
        let revealer = build_popup_revealer(&root);

        PopupEntry {
            // Store the payload used to build this row so later seeds can compare safely
            notification: notification.clone(),
            revealer: Some(revealer),
            root: Some(root),
        }
    }

    pub(super) fn build_popup_root(&mut self, notification: &NotificationView) -> gtk::Box {
        // One vertical box owns the whole popup card layout
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("unixnotis-popup-card");
        // Use the live stack width when a row is built or rebuilt
        let popup_width = self
            .popup_stack
            .width()
            .max(self.popup_stack.width_request())
            .max(1);
        root.set_size_request(popup_width, -1);
        root.set_halign(Align::Fill);
        root.set_hexpand(false);
        // New roots stay hidden until visibility logic decides otherwise
        root.set_visible(false);
        if notification.urgency == Urgency::Critical as u8 {
            // Critical rows keep the shared urgency class at the root
            root.add_css_class(hooks::shared_state::CRITICAL);
        }
        // State classes make popup theming less dependent on child selector tricks
        set_class_state(
            &root,
            hooks::popup_card::HAS_SUMMARY,
            has_visible_text(&notification.summary),
        );
        set_class_state(
            &root,
            hooks::popup_card::HAS_BODY,
            has_visible_text(&notification.body),
        );
        set_class_state(
            &root,
            hooks::popup_card::HAS_ACTIONS,
            !notification.actions.is_empty(),
        );

        // Header keeps icon, app name, and close in one stable row
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("unixnotis-popup-header-row");
        if let Some(icon) = self.build_image_widget(notification) {
            // Icon presence is exposed as a state class for theme rules
            set_class_state(&root, hooks::popup_card::HAS_ICON, true);
            icon.set_valign(Align::Center);
            icon.set_halign(Align::Start);
            icon.add_css_class("unixnotis-popup-icon");
            header.append(&icon);
        } else {
            // Missing icons also get a root class so themes can rebalance spacing
            set_class_state(&root, hooks::popup_card::NO_ICON, true);
        }
        // App name stays in the header instead of repeating the full desktop entry name
        let app = gtk::Label::new(Some(&notification.app_name));
        app.set_xalign(0.0);
        app.set_single_line_mode(true);
        app.set_ellipsize(EllipsizeMode::End);
        app.set_max_width_chars(POPUP_APP_MAX_CHARS as i32);
        app.set_text(clamp_label_text(&notification.app_name, POPUP_APP_MAX_CHARS).as_ref());
        app.add_css_class("unixnotis-popup-header");

        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("unixnotis-popup-close");
        close.set_halign(Align::End);

        // Close stays on the right edge even when the title text shrinks
        header.append(&app);
        header.append(&build_popup_header_spacer());
        header.append(&close);

        // Summary stays short and collapses when the payload has no title
        let summary = gtk::Label::new(Some(&notification.summary));
        summary.set_xalign(0.0);
        summary.set_wrap(true);
        summary.set_wrap_mode(WrapMode::WordChar);
        summary.set_ellipsize(EllipsizeMode::End);
        summary.set_lines(3);
        summary.set_max_width_chars(POPUP_SUMMARY_MAX_CHARS as i32);
        summary.add_css_class("unixnotis-popup-summary");
        update_optional_label(&summary, &notification.summary, POPUP_SUMMARY_MAX_CHARS);

        // Body follows the same bounded layout rules as the summary
        let body = gtk::Label::new(None);
        body.set_xalign(0.0);
        body.set_wrap(true);
        body.set_wrap_mode(WrapMode::WordChar);
        body.set_ellipsize(EllipsizeMode::End);
        body.set_lines(6);
        body.set_max_width_chars(POPUP_BODY_MAX_CHARS as i32);
        body.add_css_class("unixnotis-popup-body");
        update_optional_label(&body, &notification.body, POPUP_BODY_MAX_CHARS);

        // The root order is stable so CSS can assume header, summary, body, actions
        root.append(&header);
        root.append(&summary);
        root.append(&body);

        // Action buttons are only built when the payload exposes actions
        if !notification.actions.is_empty() {
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            actions.add_css_class("unixnotis-popup-actions");
            for action in notification.actions.iter().take(MAX_POPUP_ACTIONS) {
                // Button labels are clamped before GTK measures them
                let button = gtk::Button::with_label(
                    clamp_label_text(&action.label, POPUP_ACTION_LABEL_MAX_CHARS).as_ref(),
                );
                button.add_css_class("unixnotis-popup-action");
                let action_key = action.key.clone();
                let tx = self.command_tx.clone();
                let id = notification.id;
                button.connect_clicked(move |_| {
                    // Click handlers only enqueue the DBus command
                    try_send_command(
                        &tx,
                        UiCommand::InvokeAction {
                            id,
                            action_key: action_key.clone(),
                        },
                    );
                });
                actions.append(&button);
            }
            root.append(&actions);
        }

        // Close still targets the notification id even when the row is rebuilt
        let id = notification.id;
        let command_tx_close = self.command_tx.clone();
        close.connect_clicked(move |_| {
            try_send_command(&command_tx_close, UiCommand::Dismiss(id));
        });

        // Default action still fires from the rebuilt card body
        let default_action = notification
            .actions
            .iter()
            .find(|action| action.key == "default")
            .map(|action| action.key.clone());
        if let Some(action_key) = default_action {
            let gesture = gtk::GestureClick::new();
            let tx = self.command_tx.clone();
            gesture.connect_released(move |_, _, _, _| {
                // Card clicks mirror the default action button behavior
                try_send_command(
                    &tx,
                    UiCommand::InvokeAction {
                        id,
                        action_key: action_key.clone(),
                    },
                );
            });
            root.add_controller(gesture);
        }

        root
    }
}

fn build_popup_revealer(root: &gtk::Box) -> gtk::Revealer {
    // Revealers keep entry animations out of the popup list bookkeeping
    let revealer = gtk::Revealer::new();
    revealer.add_css_class("unixnotis-popup-revealer");
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_transition_duration(200);
    revealer.set_child(Some(root));
    // Visibility is driven centrally so only rows inside max_visible animate in
    revealer.set_reveal_child(false);
    revealer
}

fn build_popup_header_spacer() -> gtk::Box {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    // Spacer width takes up the slack so the trailing button does not drift
    // Plain halign on the button is not enough inside a horizontal box
    spacer.set_hexpand(popup_header_spacer_expands());
    spacer
}

pub(super) const fn popup_header_spacer_expands() -> bool {
    // Keep the alignment rule easy to test without constructing full GTK rows
    true
}

fn set_class_state(root: &gtk::Box, class_name: &str, enabled: bool) {
    if enabled {
        // Skip duplicate adds so repeated rebuilds do not churn the class list
        if !root.has_css_class(class_name) {
            root.add_css_class(class_name);
        }
    } else if root.has_css_class(class_name) {
        // Only remove classes that are really present
        root.remove_css_class(class_name);
    }
}
