//! Timed Do Not Disturb menu and compact countdown formatting

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use chrono::{Days, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use gtk::prelude::*;
use unixnotis_core::{css::hooks, DndMenuChoice, DndMenuTrigger, PanelConfig};

use crate::control::UiCommand;
use crate::ui::try_send_command;

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
    secondary_click: gtk::GestureClick,
    long_press: gtk::GestureLongPress,
    key_controller: gtk::EventControllerKey,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
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
    config: &PanelConfig,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) -> DndDurationMenu {
    // The DND toggle owns this popover without adding a separate arrow button
    let popover = gtk::Popover::new();
    popover.add_css_class(hooks::dnd_menu::ROOT);
    // A flat edge aligns with the action row without GTK's detached-looking arrow notch
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    popover.set_parent(dnd_toggle);
    let (secondary_click, long_press, key_controller) =
        connect_dnd_menu_inputs(dnd_toggle, &popover);
    let menu = DndDurationMenu {
        popover,
        secondary_click,
        long_press,
        key_controller,
        command_tx,
    };
    menu.apply_config(config);
    menu
}

impl DndDurationMenu {
    pub(in crate::ui) fn apply_config(&self, config: &PanelConfig) {
        self.popover.popdown();
        self.popover.set_child(Some(&build_choice_box(
            &config.dnd_menu_choices,
            &self.command_tx,
            &self.popover,
        )));

        // Installed controllers can be disabled safely without replacing GTK ownership
        let has_choices = !config.dnd_menu_choices.is_empty();
        set_controller_enabled(
            &self.secondary_click,
            has_choices
                && config
                    .dnd_menu_triggers
                    .contains(&DndMenuTrigger::RightClick),
        );
        set_controller_enabled(
            &self.long_press,
            has_choices
                && config
                    .dnd_menu_triggers
                    .contains(&DndMenuTrigger::LongPress),
        );
        set_controller_enabled(
            &self.key_controller,
            has_choices && config.dnd_menu_triggers.contains(&DndMenuTrigger::Keyboard),
        );
    }
}

fn build_choice_box(
    choices: &[DndMenuChoice],
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    popover: &gtk::Popover,
) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    container.add_css_class(hooks::dnd_menu::CONTENT);

    // A small heading explains the time choices without repeating DND state
    let title = gtk::Label::new(Some("Pause notifications"));
    title.set_xalign(0.0);
    title.add_css_class(hooks::dnd_menu::TITLE);
    container.append(&title);

    for choice in choices {
        if matches!(choice, DndMenuChoice::Indefinite { .. }) {
            // A real separator stays crisp without borrowing a button border
            let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
            separator.add_css_class(hooks::dnd_menu::SEPARATOR);
            container.append(&separator);
        }
        // Left-aligned rows scan faster than a stack of centered default buttons
        let button = gtk::Button::with_label(choice.label());
        if let Some(label) = button.child().and_downcast::<gtk::Label>() {
            label.set_xalign(0.0);
            label.set_hexpand(true);
        }
        button.add_css_class(hooks::dnd_menu::CHOICE);
        if matches!(choice, DndMenuChoice::Indefinite { .. }) {
            // Indefinite mode is separated because it has no automatic resume time
            button.add_css_class(hooks::dnd_menu::INDEFINITE);
        }
        connect_choice_button(&button, choice, command_tx, popover);
        container.append(&button);
    }
    container
}

fn connect_choice_button(
    button: &gtk::Button,
    choice: &DndMenuChoice,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
    popover: &gtk::Popover,
) {
    let choice = choice.clone();
    let command_tx = command_tx.clone();
    let menu = popover.downgrade();
    button.connect_clicked(move |_| {
        match choice {
            DndMenuChoice::Duration { minutes, .. } => {
                // Sanitized minute values still use saturation at the clock boundary
                let seconds = i64::from(minutes).saturating_mul(60);
                let expires_at = Utc::now().timestamp().saturating_add(seconds);
                try_send_command(&command_tx, UiCommand::SetDndUntil(expires_at));
            }
            DndMenuChoice::Tomorrow { hour, minute, .. } => {
                if let Some(expires_at) = next_day_deadline(u32::from(hour), u32::from(minute)) {
                    try_send_command(&command_tx, UiCommand::SetDndUntil(expires_at));
                } else {
                    // Config values stay out of logs because the stable failure category is enough
                    tracing::warn!("could not resolve configured next-day DND deadline");
                }
            }
            DndMenuChoice::Indefinite { .. } => {
                // Indefinite enablement deliberately replaces any timed deadline
                try_send_command(&command_tx, UiCommand::SetDnd(true));
            }
        }
        if let Some(menu) = menu.upgrade() {
            menu.popdown();
        }
    });
}

fn set_controller_enabled(controller: &impl IsA<gtk::EventController>, enabled: bool) {
    let phase = if enabled {
        gtk::PropagationPhase::Bubble
    } else {
        gtk::PropagationPhase::None
    };
    controller.set_propagation_phase(phase);
}

fn connect_dnd_menu_inputs(
    dnd_toggle: &gtk::ToggleButton,
    popover: &gtk::Popover,
) -> (
    gtk::GestureClick,
    gtk::GestureLongPress,
    gtk::EventControllerKey,
) {
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
    dnd_toggle.add_controller(secondary_click.clone());

    let long_press = gtk::GestureLongPress::new();
    let press_menu = popover.downgrade();
    long_press.connect_pressed(move |gesture, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        if let Some(menu) = press_menu.upgrade() {
            menu.popup();
        }
    });
    dnd_toggle.add_controller(long_press.clone());

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
    dnd_toggle.add_controller(key_controller.clone());
    (secondary_click, long_press, key_controller)
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

fn next_day_deadline(hour: u32, minute: u32) -> Option<i64> {
    let now = Local::now();
    // Construct the local clock value separately from the next calendar date
    let local_time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    let date = tomorrow_date(now.date_naive())?;
    match Local.from_local_datetime(&date.and_time(local_time)) {
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
