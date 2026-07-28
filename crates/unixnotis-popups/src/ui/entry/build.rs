//! Popup entry lifecycle and high-level card assembly

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{hooks, NotificationView};
use unixnotis_ui::CutCorner;

use super::super::window::refresh_popup_input_region;
use super::super::UiState;
use super::builders::{
    build_action_row, build_close_button, build_inline_reply, build_popup_content,
};
use super::commands::try_send_command;
use super::presentation::PopupEntryViewModel;
use crate::dbus::UiCommand;

pub(in crate::ui) struct PopupEntry {
    // Keep the last payload so seed reconcile can detect real content changes
    pub(in crate::ui) notification: NotificationView,
    // Hidden backlog rows stay lightweight until they enter the visible slice
    pub(in crate::ui) revealer: Option<gtk::Revealer>,
    pub(in crate::ui) root: Option<gtk::Box>,
}

impl PopupEntry {
    pub(in crate::ui) const fn queued(notification: NotificationView) -> Self {
        // Backlog rows start as plain data and only grow GTK nodes when they become visible
        Self {
            notification,
            revealer: None,
            root: None,
        }
    }

    pub(in crate::ui) const fn is_materialized(&self) -> bool {
        // Both widgets must exist before stack operations can touch this row safely
        self.revealer.is_some() && self.root.is_some()
    }
}

impl UiState {
    pub(in crate::ui) fn build_popup_entry(
        &mut self,
        notification: &NotificationView,
    ) -> PopupEntry {
        // Build the GTK row first so the revealer always wraps a ready child
        let root = self.build_popup_root(notification);
        let revealer = self.build_popup_revealer(&root);

        PopupEntry {
            // Store the payload used to build this row so later seeds can compare safely
            notification: notification.clone(),
            revealer: Some(revealer),
            root: Some(root),
        }
    }

    pub(in crate::ui) fn build_popup_root(&mut self, notification: &NotificationView) -> gtk::Box {
        let view = PopupEntryViewModel::for_notification(notification);
        let root = build_card_root(self, &view);
        let close = build_close_button();
        let rendered = build_popup_content(self, notification, &view, &close);

        // Builder results feed stable state classes used by user themes
        set_class_state(&root, hooks::popup_card::HAS_ICON, rendered.has_icon);
        set_class_state(&root, hooks::popup_card::NO_ICON, !rendered.has_icon);
        set_class_state(&root, hooks::popup_card::HAS_IMAGE, rendered.has_image);
        root.append(&rendered.widget);

        if let Some(reply) = build_inline_reply(notification, &view, &self.command_tx) {
            root.append(&reply);
        }
        if let Some(actions) = build_action_row(&self.command_tx, notification.id, &view) {
            root.append(&actions);
        }

        connect_close_action(&close, notification.id, &self.command_tx);
        connect_default_action(&root, notification.id, &view, &self.command_tx);
        root
    }

    fn build_popup_revealer(&self, root: &gtk::Box) -> gtk::Revealer {
        // Revealers keep entry animations out of the popup list bookkeeping
        let revealer = gtk::Revealer::new();
        revealer.add_css_class("unixnotis-popup-revealer");
        if self.config.panel.reduced_motion {
            // Reduced motion keeps state changes immediate without hiding content
            revealer.set_transition_type(gtk::RevealerTransitionType::None);
            revealer.set_transition_duration(0);
        } else {
            // A short fade avoids geometry-heavy card animations
            revealer.set_transition_type(gtk::RevealerTransitionType::Crossfade);
            revealer.set_transition_duration(200);
        }
        // The shared primitive clips the full styled popup instead of approximating the corners
        let plate = CutCorner::new(root, self.config.theme.notification_corners);
        revealer.set_child(Some(&plate));
        // Visibility is driven centrally so only rows inside max_visible animate in
        revealer.set_reveal_child(false);

        let popup_window = self.popup_window.clone();
        let popup_stack = self.popup_stack.clone();
        let popup_input_region = self.popup_input_region.clone();
        revealer.connect_notify_local(Some("child-revealed"), move |_, _| {
            // Refresh after reveal so action rows never inherit an earlier empty input region
            refresh_popup_input_region(&popup_window, &popup_stack, &popup_input_region);
        });

        revealer
    }
}

fn build_card_root(state: &UiState, view: &PopupEntryViewModel) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.add_css_class("unixnotis-popup-card");
    root.add_css_class(view.kind.css_class());
    root.add_css_class(view.trust.level.css_class());

    // Use the live stack width when a row is built or rebuilt
    let popup_width = state
        .popup_stack
        .width()
        .max(state.popup_stack.width_request())
        .max(1);
    root.set_size_request(popup_width, -1);
    root.set_halign(Align::Fill);
    root.set_hexpand(false);
    // New roots stay hidden until visibility logic decides otherwise
    root.set_visible(false);

    if view.critical {
        root.add_css_class(hooks::shared_state::CRITICAL);
    }
    set_class_state(
        &root,
        hooks::popup_card::HAS_SUMMARY,
        !view.title.trim().is_empty(),
    );
    set_class_state(&root, hooks::popup_card::HAS_BODY, view.body.is_some());
    set_class_state(
        &root,
        hooks::popup_card::HAS_ACTIONS,
        view.trust.reply == super::presentation::ReplyPresentation::Available
            || !view.primary_actions.is_empty()
            || !view.overflow_actions.is_empty(),
    );
    root
}

fn connect_close_action(
    close: &gtk::Button,
    notification_id: u32,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) {
    let command_tx = command_tx.clone();
    close.connect_clicked(move |_| {
        // Dismissal remains independent from application-owned action policy
        try_send_command(&command_tx, UiCommand::Dismiss(notification_id));
    });
}

fn connect_default_action(
    root: &gtk::Box,
    notification_id: u32,
    view: &PopupEntryViewModel,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) {
    let Some(action_key) = view
        .primary_actions
        .iter()
        .chain(&view.overflow_actions)
        .find(|action| action.key == "default")
        .map(|action| action.key.clone())
    else {
        return;
    };

    let gesture = gtk::GestureClick::new();
    // Default card actions only belong to plain card clicks
    gesture.set_button(1);
    let root_weak = root.downgrade();
    let tx = command_tx.clone();
    gesture.connect_released(move |_, _, x, y| {
        let Some(root) = root_weak.upgrade() else {
            return;
        };
        if picked_widget_blocks_default_action(root.pick(x, y, gtk::PickFlags::DEFAULT)) {
            return;
        }
        // The presentation model already removed actions with weak provenance
        try_send_command(
            &tx,
            UiCommand::InvokeAction {
                id: notification_id,
                action_key: action_key.clone(),
            },
        );
    });
    root.add_controller(gesture);
}

fn picked_widget_blocks_default_action(mut widget: Option<gtk::Widget>) -> bool {
    while let Some(current) = widget {
        if widget_type_blocks_default_action(current.type_()) {
            return true;
        }
        widget = current.parent();
    }
    false
}

fn widget_type_blocks_default_action(widget_type: gtk::glib::Type) -> bool {
    // Button clicks should always stay owned by the button widget subtree
    widget_type.is_a(gtk::Button::static_type())
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

#[cfg(test)]
#[path = "tests/build.rs"]
mod tests;
