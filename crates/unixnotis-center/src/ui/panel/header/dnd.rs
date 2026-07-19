//! Timed Do Not Disturb menu and compact countdown formatting

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use chrono::{Days, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use gtk::prelude::*;

use crate::control::UiCommand;
use crate::ui::try_send_command;

const MORNING_HOUR: u32 = 8;
const DND_DURATION_CHOICES: [(&str, i64); 3] =
    [("30 minutes", 1_800), ("1 hour", 3_600), ("2 hours", 7_200)];

// Context-menu keys use one small decision type so GTK behavior stays explicit
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DndMenuKeyAction {
    Open,
    Ignore,
}

pub(in crate::ui) struct DndCountdown {
    source: Option<gtk::glib::SourceId>,
    active: Rc<Cell<bool>>,
}

pub(in crate::ui) struct DndDurationMenu {
    // A manually parented popover needs an explicit owner to detach it before panel teardown
    popover: gtk::Popover,
}

impl Drop for DndDurationMenu {
    fn drop(&mut self) {
        // GTK does not automatically detach popovers added with set_parent
        if self.popover.parent().is_some() {
            self.popover.unparent();
        }
    }
}

impl DndCountdown {
    fn remove_active_source(&mut self) {
        if self.active.replace(false) {
            // GLib removal is valid only while the callback still owns a live source
            if let Some(source) = self.source.take() {
                source.remove();
            }
        }
    }
}

impl Drop for DndCountdown {
    fn drop(&mut self) {
        // Dropping panel state must not leave a callback retaining the countdown label
        self.remove_active_source();
    }
}

pub(in crate::ui) fn connect_dnd_menu(
    dnd_toggle: &gtk::ToggleButton,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) -> DndDurationMenu {
    // The DND toggle owns this popover without adding a separate arrow button
    let popover = gtk::Popover::new();
    popover.set_autohide(true);
    let choices = gtk::Box::new(gtk::Orientation::Vertical, 2);

    // Common relative choices share one absolute-deadline command path
    for (label, seconds) in DND_DURATION_CHOICES {
        let button = gtk::Button::with_label(label);
        let tx = command_tx.clone();
        let menu = popover.downgrade();
        button.connect_clicked(move |_| {
            // Saturation keeps an abnormal system clock from wrapping the deadline
            let expires_at = Utc::now().timestamp().saturating_add(seconds);
            try_send_command(&tx, UiCommand::SetDndUntil(expires_at));
            if let Some(menu) = menu.upgrade() {
                menu.popdown();
            }
        });
        choices.append(&button);
    }

    // Morning follows the next calendar day rather than a fixed 24-hour duration
    let morning = gtk::Button::with_label("Until tomorrow morning");
    let morning_tx = command_tx.clone();
    let morning_menu = popover.downgrade();
    morning.connect_clicked(move |_| {
        if let Some(expires_at) = next_morning_deadline() {
            try_send_command(&morning_tx, UiCommand::SetDndUntil(expires_at));
        } else {
            tracing::warn!("could not resolve the next local 08:00 DND deadline");
        }
        if let Some(menu) = morning_menu.upgrade() {
            menu.popdown();
        }
    });
    choices.append(&morning);

    // Indefinite enablement deliberately replaces any existing timed deadline
    let indefinite = gtk::Button::with_label("Indefinitely");
    let indefinite_menu = popover.downgrade();
    indefinite.connect_clicked(move |_| {
        try_send_command(&command_tx, UiCommand::SetDnd(true));
        if let Some(menu) = indefinite_menu.upgrade() {
            menu.popdown();
        }
    });
    choices.append(&indefinite);

    popover.set_child(Some(&choices));
    popover.set_parent(dnd_toggle);
    connect_dnd_menu_inputs(dnd_toggle, &popover);
    DndDurationMenu { popover }
}

fn connect_dnd_menu_inputs(dnd_toggle: &gtk::ToggleButton, popover: &gtk::Popover) {
    let secondary_click = gtk::GestureClick::new();
    // Secondary click keeps the primary click dedicated to immediate toggling
    secondary_click.set_button(3);
    let click_menu = popover.downgrade();
    secondary_click.connect_pressed(move |gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        if let Some(menu) = click_menu.upgrade() {
            menu.popup();
        }
    });
    dnd_toggle.add_controller(secondary_click);

    let long_press = gtk::GestureLongPress::new();
    let press_menu = popover.downgrade();
    long_press.connect_pressed(move |gesture, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        if let Some(menu) = press_menu.upgrade() {
            menu.popup();
        }
    });
    dnd_toggle.add_controller(long_press);

    let key_controller = gtk::EventControllerKey::new();
    let key_menu = popover.downgrade();
    key_controller.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(menu) = key_menu.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        match dnd_menu_key_action(key, modifiers) {
            DndMenuKeyAction::Open => {
                menu.popup();
                gtk::glib::Propagation::Stop
            }
            DndMenuKeyAction::Ignore => gtk::glib::Propagation::Proceed,
        }
    });
    dnd_toggle.add_controller(key_controller);
}

fn dnd_menu_key_action(key: gtk::gdk::Key, modifiers: gtk::gdk::ModifierType) -> DndMenuKeyAction {
    if key == gtk::gdk::Key::Menu
        || (key == gtk::gdk::Key::F10 && modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK))
    {
        DndMenuKeyAction::Open
    } else {
        DndMenuKeyAction::Ignore
    }
}

pub(in crate::ui) fn update_dnd_status(label: &gtk::Label, expires_at: i64) {
    // One helper keeps immediate and timer-driven label updates identical
    let text = format_dnd_remaining(expires_at, Utc::now().timestamp());
    label.set_visible(!text.is_empty());
    label.set_text(&text);
}

pub(in crate::ui) fn start_dnd_countdown(label: &gtk::Label, expires_at: i64) -> DndCountdown {
    // GTK owns the callback on its main context while UiState owns the source id
    let label = label.clone();
    let active = Rc::new(Cell::new(true));
    let callback_active = active.clone();
    let source = gtk::glib::timeout_add_local(Duration::from_secs(30), move || {
        update_dnd_status(&label, expires_at);
        let flow = countdown_control_flow(expires_at, Utc::now().timestamp());
        if flow == gtk::glib::ControlFlow::Break {
            // Mark the ID inactive before GLib destroys it after this callback
            callback_active.set(false);
        }
        flow
    });
    DndCountdown {
        source: Some(source),
        active,
    }
}

const fn countdown_control_flow(expires_at: i64, now: i64) -> gtk::glib::ControlFlow {
    if expires_at <= now {
        gtk::glib::ControlFlow::Break
    } else {
        gtk::glib::ControlFlow::Continue
    }
}

fn format_dnd_remaining(expires_at: i64, now: i64) -> String {
    let remaining = expires_at.saturating_sub(now);
    if remaining <= 0 {
        return String::new();
    }
    // Round upward so a positive remainder never appears as zero minutes
    let minutes = (remaining.saturating_add(59)) / 60;
    if minutes < 60 {
        return format!("· {minutes}m");
    }
    let hours = minutes / 60;
    let trailing_minutes = minutes % 60;
    if trailing_minutes == 0 {
        format!("· {hours}h")
    } else {
        format!("· {hours}h {trailing_minutes}m")
    }
}

fn next_morning_deadline() -> Option<i64> {
    let now = Local::now();
    // Construct the local clock value separately from the next calendar date
    let morning = NaiveTime::from_hms_opt(MORNING_HOUR, 0, 0)?;
    let date = tomorrow_date(now.date_naive())?;
    match Local.from_local_datetime(&date.and_time(morning)) {
        chrono::LocalResult::Single(value) => Some(value.timestamp()),
        // The earliest occurrence is sufficient because the whole date is in the future
        chrono::LocalResult::Ambiguous(first, _) => Some(first.timestamp()),
        chrono::LocalResult::None => None,
    }
}

const fn tomorrow_date(today: NaiveDate) -> Option<NaiveDate> {
    today.checked_add_days(Days::new(1))
}

#[cfg(test)]
#[path = "tests/dnd.rs"]
mod tests;
