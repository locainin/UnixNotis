//! Popup entry construction and UI action wiring.
//!
//! Keeps widget assembly separate from popup list bookkeeping.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::prelude::*;
use gtk::Align;
use gtk::pango;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tracing::{debug, warn};
use unixnotis_core::{NotificationView, Urgency};

use crate::dbus::UiCommand;

use super::UiState;

pub(super) struct PopupEntry {
    pub(super) revealer: gtk::Revealer,
    pub(super) root: gtk::Box,
}

impl UiState {
    pub(super) fn build_popup_entry(&mut self, notification: &NotificationView) -> PopupEntry {
        let revealer = gtk::Revealer::new();
        revealer.add_css_class("unixnotis-popup-revealer");
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(200);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("unixnotis-popup-card");
        if notification.urgency == Urgency::Critical as u8 {
            root.add_css_class("critical");
        }

        // Header row groups app icon, name, and close action.
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("unixnotis-popup-header-row");
        if let Some(icon) = self.build_image_widget(notification) {
            icon.set_valign(Align::Center);
            icon.set_halign(Align::Start);
            icon.add_css_class("unixnotis-popup-icon");
            header.append(&icon);
        }
        let app = gtk::Label::new(Some(&notification.app_name));
        app.set_xalign(0.0);
        app.add_css_class("unixnotis-popup-header");

        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("unixnotis-popup-close");
        close.set_halign(Align::End);

        header.append(&app);
        header.append(&gtk::Box::new(gtk::Orientation::Horizontal, 1));
        header.append(&close);

        // Summary line mirrors the notification title for quick scanning.
        let summary = gtk::Label::new(Some(&notification.summary));
        summary.set_xalign(0.0);
        summary.set_wrap(true);
        summary.add_css_class("unixnotis-popup-summary");

        // Body uses markup to preserve formatting supplied by the notification.
        let body = gtk::Label::new(None);
        body.set_xalign(0.0);
        body.set_wrap(true);
        body.add_css_class("unixnotis-popup-body");
        set_label_markup(&body, &notification.body);

        root.append(&header);
        root.append(&summary);
        root.append(&body);

        // Action buttons map user clicks back into D-Bus action invocations.
        if !notification.actions.is_empty() {
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            actions.add_css_class("unixnotis-popup-actions");
            for action in &notification.actions {
                let button = gtk::Button::with_label(&action.label);
                button.add_css_class("unixnotis-popup-action");
                let action_key = action.key.clone();
                let tx = self.command_tx.clone();
                let id = notification.id;
                button.connect_clicked(move |_| {
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

        // Close button requests dismissal for the specific notification id.
        let id = notification.id;
        let command_tx_close = self.command_tx.clone();
        close.connect_clicked(move |_| {
            try_send_command(&command_tx_close, UiCommand::Dismiss(id));
        });

        // Default action activates when the popup body is clicked.
        let default_action = notification
            .actions
            .iter()
            .find(|action| action.key == "default")
            .map(|action| action.key.clone());
        if let Some(action_key) = default_action {
            let gesture = gtk::GestureClick::new();
            let tx = self.command_tx.clone();
            gesture.connect_released(move |_, _, _, _| {
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

        revealer.set_child(Some(&root));
        revealer.set_reveal_child(true);

        PopupEntry { revealer, root }
    }
}

fn set_label_markup(label: &gtk::Label, body: &str) {
    if body.is_empty() {
        label.set_text("");
        return;
    }
    // Validate markup before applying it to avoid panics and malformed render state.
    // Use '\0' to disable accelerator parsing and keep validation strict.
    // Fallback to plain text so notifications remain readable.
    if let Err(err) = pango::parse_markup(body, '\0') {
        if should_warn_invalid_markup() {
            warn!(?err, "invalid notification markup; falling back to plain text");
        }
        label.set_text(body);
        return;
    }
    label.set_markup(body);
}

fn should_warn_invalid_markup() -> bool {
    // Rate-limit malformed markup warnings to avoid log spam from noisy apps.
    const WARN_INTERVAL_SECS: u64 = 30;
    static LAST_WARN: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = LAST_WARN.load(Ordering::Relaxed);
    if now.saturating_sub(last) >= WARN_INTERVAL_SECS {
        LAST_WARN.store(now, Ordering::Relaxed);
        return true;
    }
    false
}

fn try_send_command(tx: &Sender<UiCommand>, command: UiCommand) {
    // Avoid blocking the GTK thread; fall back to async send if the queue is full.
    match tx.try_send(command) {
        Ok(()) => {}
        Err(TrySendError::Full(command)) => {
            enqueue_fallback(tx, command);
        }
        Err(TrySendError::Closed(command)) => {
            debug!(?command, "command channel closed; dropping UI action");
        }
    }
}

fn enqueue_fallback(tx: &Sender<UiCommand>, command: UiCommand) {
    // Use a bounded fallback queue to keep user actions flowing without spawning
    // unbounded async tasks when the main command channel is saturated.
    const FALLBACK_QUEUE_CAPACITY: usize = 32;
    static FALLBACK: OnceLock<async_channel::Sender<UiCommand>> = OnceLock::new();

    let fallback = FALLBACK.get_or_init(|| {
        let (fallback_tx, fallback_rx) = async_channel::bounded(FALLBACK_QUEUE_CAPACITY);
        let target = tx.clone();
        gtk::glib::MainContext::default().spawn_local(async move {
            while let Ok(cmd) = fallback_rx.recv().await {
                if target.send(cmd).await.is_err() {
                    break;
                }
            }
        });
        fallback_tx
    });

    if fallback.try_send(command).is_err() {
        debug!("popup command fallback queue full; dropping UI action");
    }
}
