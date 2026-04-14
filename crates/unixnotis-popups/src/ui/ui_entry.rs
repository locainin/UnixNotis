//! Popup entry construction and UI action wiring.
//!
//! Keeps widget assembly separate from popup list bookkeeping.

use std::borrow::Cow;
use std::sync::OnceLock;

use gtk::pango::{EllipsizeMode, WrapMode};
use gtk::prelude::*;
use gtk::Align;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tracing::debug;
use unixnotis_core::{hooks, NotificationView, Urgency};

use crate::dbus::UiCommand;

use super::UiState;

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
// Header/app title stays single-line and clipped at this length
const POPUP_APP_MAX_CHARS: usize = 40;
// Summary is visually dominant but still bounded to avoid tall cards
const POPUP_SUMMARY_MAX_CHARS: usize = 120;
// Body keeps enough context while preventing oversized popup growth
const POPUP_BODY_MAX_CHARS: usize = 320;
// Action labels stay short so button row width remains predictable
const POPUP_ACTION_LABEL_MAX_CHARS: usize = 14;

struct OptionalLabelState<'a> {
    // Empty rows should disappear instead of leaving stray spacing behind
    visible: bool,
    // Reuse borrowed text when possible so empty checks stay cheap
    text: Cow<'a, str>,
}

impl UiState {
    pub(super) fn build_popup_entry(&mut self, notification: &NotificationView) -> PopupEntry {
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
            set_class_state(&root, hooks::popup_card::HAS_ICON, true);
            icon.set_valign(Align::Center);
            icon.set_halign(Align::Start);
            icon.add_css_class("unixnotis-popup-icon");
            header.append(&icon);
        } else {
            set_class_state(&root, hooks::popup_card::NO_ICON, true);
        }
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

        root.append(&header);
        root.append(&summary);
        root.append(&body);

        // Action buttons are only built when the payload exposes actions
        if !notification.actions.is_empty() {
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            actions.add_css_class("unixnotis-popup-actions");
            for action in notification.actions.iter().take(MAX_POPUP_ACTIONS) {
                let button = gtk::Button::with_label(
                    clamp_label_text(&action.label, POPUP_ACTION_LABEL_MAX_CHARS).as_ref(),
                );
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

const fn popup_header_spacer_expands() -> bool {
    // Keep the alignment rule easy to test without constructing full GTK rows
    true
}

fn update_optional_label(label: &gtk::Label, text: &str, max_chars: usize) {
    // Build the layout decision first so empty-text handling stays identical
    // for both summary and body rows
    let state = optional_label_state(text, max_chars);
    label.set_visible(state.visible);
    label.set_text(state.text.as_ref());
}

fn optional_label_state(text: &str, max_chars: usize) -> OptionalLabelState<'_> {
    if !has_visible_text(text) {
        // Empty text rows stay hidden so the card does not keep dead spacing
        return OptionalLabelState {
            visible: false,
            text: Cow::Borrowed(""),
        };
    }
    OptionalLabelState {
        visible: true,
        // Clamp before the label sees the text so layout work stays bounded
        text: clamp_label_text(text, max_chars),
    }
}

fn has_visible_text(text: &str) -> bool {
    // Visibility depends on real content, not just raw string length
    text.chars().any(|ch| !ch.is_whitespace())
}

fn set_class_state(root: &gtk::Box, class_name: &str, enabled: bool) {
    if enabled {
        if !root.has_css_class(class_name) {
            root.add_css_class(class_name);
        }
    } else if root.has_css_class(class_name) {
        root.remove_css_class(class_name);
    }
}

fn clamp_label_text(text: &str, max_chars: usize) -> Cow<'_, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }
    // char_indices preserves UTF-8 boundaries during truncation
    for (chars, (idx, _)) in text.char_indices().enumerate() {
        if chars == max_chars {
            let mut clamped = String::with_capacity(idx + 3);
            clamped.push_str(&text[..idx]);
            clamped.push('…');
            return Cow::Owned(clamped);
        }
    }
    Cow::Borrowed(text)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_header_spacer_expands_to_hold_close_alignment() {
        // The tested rule is the important part, not the GTK object itself
        assert!(popup_header_spacer_expands());
    }

    #[test]
    fn popup_summary_row_hides_when_text_is_empty() {
        // Empty summaries should not reserve vertical space above the body
        let state = optional_label_state("", POPUP_SUMMARY_MAX_CHARS);

        assert!(!state.visible);
        assert!(state.text.is_empty());
    }

    #[test]
    fn popup_body_row_hides_when_text_is_empty() {
        // Body-less notifications should render as header plus summary only
        let state = optional_label_state("", POPUP_BODY_MAX_CHARS);

        assert!(!state.visible);
        assert!(state.text.is_empty());
    }

    #[test]
    fn popup_body_row_hides_when_text_is_only_whitespace() {
        // Space-only bodies should not leave a blank band in the popup card
        let state = optional_label_state("\n\t ", POPUP_BODY_MAX_CHARS);

        assert!(!state.visible);
        assert!(state.text.is_empty());
    }

    #[test]
    fn popup_summary_row_shows_when_text_has_real_content() {
        // Real text should stay intact even when it has leading whitespace
        let state = optional_label_state("  hello  ", POPUP_SUMMARY_MAX_CHARS);

        assert!(state.visible);
        assert_eq!(state.text.as_ref(), "  hello  ");
    }
}
