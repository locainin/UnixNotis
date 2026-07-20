use std::cell::Cell;
use std::rc::Rc;

use chrono::{Local, NaiveDate, TimeZone, Timelike, Utc};
use gtk::prelude::*;

use super::{
    connect_dnd_menu, countdown_control_flow, dnd_menu_key_action, format_dnd_remaining,
    next_day_deadline, tomorrow_date, update_dnd_status, DndCountdown, DndMenuKeyAction,
};
use crate::control::UiCommand;
use unixnotis_core::{DndMenuChoice, DndMenuTrigger, PanelConfig};

#[test]
fn remaining_time_is_hidden_after_expiry_and_rounded_up_before_it() {
    assert_eq!(format_dnd_remaining(100, 100), "");
    assert_eq!(format_dnd_remaining(99, 100), "");
    assert_eq!(format_dnd_remaining(101, 100), "· 1m");
    assert_eq!(format_dnd_remaining(100 + 47 * 60, 100), "· 47m");
}

#[test]
fn remaining_time_keeps_hours_compact_without_losing_partial_hour() {
    assert_eq!(format_dnd_remaining(100 + 60 * 60, 100), "· 1h");
    assert_eq!(
        format_dnd_remaining(100 + 2 * 60 * 60 + 5 * 60, 100),
        "· 2h 5m"
    );
}

#[test]
fn next_day_choice_uses_the_next_local_calendar_date() {
    let today = NaiveDate::from_ymd_opt(2026, 7, 18).expect("valid date");

    assert_eq!(tomorrow_date(today), NaiveDate::from_ymd_opt(2026, 7, 19));
    assert!(next_day_deadline(24, 0).is_none());
    assert!(next_day_deadline(8, 60).is_none());
}

#[test]
fn countdown_stops_at_the_deadline_and_continues_only_while_future() {
    assert_eq!(
        countdown_control_flow(100, 99),
        gtk::glib::ControlFlow::Continue
    );
    assert_eq!(
        countdown_control_flow(100, 100),
        gtk::glib::ControlFlow::Break
    );
    assert_eq!(
        countdown_control_flow(100, 101),
        gtk::glib::ControlFlow::Break
    );
}

#[test]
fn duration_menu_accepts_standard_keyboard_context_actions_only() {
    assert_eq!(
        dnd_menu_key_action(gtk::gdk::Key::Menu, gtk::gdk::ModifierType::empty()),
        DndMenuKeyAction::Open
    );
    assert_eq!(
        dnd_menu_key_action(gtk::gdk::Key::F10, gtk::gdk::ModifierType::SHIFT_MASK),
        DndMenuKeyAction::Open
    );
    assert_eq!(
        dnd_menu_key_action(gtk::gdk::Key::F10, gtk::gdk::ModifierType::empty()),
        DndMenuKeyAction::Ignore
    );
    assert_eq!(
        dnd_menu_key_action(gtk::gdk::Key::space, gtk::gdk::ModifierType::SHIFT_MASK),
        DndMenuKeyAction::Ignore
    );
}

#[gtk::test]
fn default_duration_menu_enables_only_right_click_and_keeps_stock_choices() {
    let toggle = gtk::ToggleButton::new();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(8);
    let config = PanelConfig::default();

    let menu_owner = connect_dnd_menu(&toggle, &config, command_tx);

    let popover = attached_popover(&toggle);
    assert!(popover.is_autohide());
    assert!(popover.has_css_class("unixnotis-dnd-menu"));
    assert!(popover
        .child()
        .is_some_and(|child| child.has_css_class("unixnotis-dnd-menu-content")));
    assert_eq!(
        menu_buttons(&popover)
            .iter()
            .filter_map(gtk::Button::label)
            .collect::<Vec<_>>(),
        vec![
            "30 minutes",
            "1 hour",
            "2 hours",
            "Until tomorrow morning",
            "Indefinitely",
        ]
    );
    assert!(menu_buttons(&popover)
        .iter()
        .all(|button| button.has_css_class("unixnotis-dnd-menu-choice")));
    assert!(!menu_buttons(&popover)[3].has_css_class("unixnotis-dnd-menu-choice-indefinite"));
    assert!(menu_buttons(&popover)[4].has_css_class("unixnotis-dnd-menu-choice-indefinite"));
    assert!(menu_separator(&popover).has_css_class("unixnotis-dnd-menu-separator"));

    assert_eq!(menu_owner.secondary_click.button(), 3);
    assert_eq!(
        menu_owner.secondary_click.propagation_phase(),
        gtk::PropagationPhase::Bubble
    );
    assert_eq!(
        menu_owner.long_press.propagation_phase(),
        gtk::PropagationPhase::None
    );
    assert_eq!(
        menu_owner.key_controller.propagation_phase(),
        gtk::PropagationPhase::None
    );
}

#[gtk::test]
fn duration_menu_applies_custom_inputs_and_choices_without_reconnecting() {
    let toggle = gtk::ToggleButton::new();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(8);
    let menu_owner = connect_dnd_menu(&toggle, &PanelConfig::default(), command_tx);
    let config = PanelConfig {
        dnd_menu_triggers: vec![DndMenuTrigger::LongPress, DndMenuTrigger::Keyboard],
        dnd_menu_choices: vec![DndMenuChoice::Duration {
            label: "Focus block".to_string(),
            minutes: 45,
        }],
        ..PanelConfig::default()
    };

    menu_owner.apply_config(&config);

    assert_eq!(
        menu_buttons(&menu_owner.popover)
            .iter()
            .filter_map(gtk::Button::label)
            .collect::<Vec<_>>(),
        vec!["Focus block"]
    );
    assert_eq!(
        menu_owner.secondary_click.propagation_phase(),
        gtk::PropagationPhase::None
    );
    assert_eq!(
        menu_owner.long_press.propagation_phase(),
        gtk::PropagationPhase::Bubble
    );
    assert_eq!(
        menu_owner.key_controller.propagation_phase(),
        gtk::PropagationPhase::Bubble
    );
}

#[gtk::test]
fn dropping_duration_menu_owner_detaches_the_manually_parented_popover() {
    let toggle = gtk::ToggleButton::new();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let menu_owner = connect_dnd_menu(&toggle, &PanelConfig::default(), command_tx);
    let popover = menu_owner.popover.clone();

    assert_eq!(
        popover.parent().as_ref(),
        Some(toggle.upcast_ref::<gtk::Widget>())
    );
    drop(menu_owner);
    assert!(popover.parent().is_none());
    drop(toggle);
}

#[gtk::test]
fn duration_menu_buttons_send_their_exact_deadlines_and_indefinite_state() {
    let toggle = gtk::ToggleButton::new();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(8);
    let _menu_owner = connect_dnd_menu(&toggle, &PanelConfig::default(), command_tx);
    let buttons = menu_buttons(&attached_popover(&toggle));

    for (minutes, button) in [30_i64, 60, 120].iter().zip(&buttons[..3]) {
        let seconds = minutes.saturating_mul(60);
        let before = Utc::now().timestamp();
        button.emit_clicked();
        let after = Utc::now().timestamp();
        let UiCommand::SetDndUntil(expires_at) = command_rx
            .try_recv()
            .expect("duration command should queue")
        else {
            panic!("expected timed DND command");
        };
        assert!(expires_at >= before.saturating_add(seconds));
        assert!(expires_at <= after.saturating_add(seconds));
    }

    let before_morning = Local::now();
    buttons[3].emit_clicked();
    let UiCommand::SetDndUntil(morning_deadline) =
        command_rx.try_recv().expect("morning command should queue")
    else {
        panic!("expected morning DND command");
    };
    let morning = Local
        .timestamp_opt(morning_deadline, 0)
        .single()
        .expect("morning deadline should map to one local time");
    assert!(morning.date_naive() > before_morning.date_naive());
    assert_eq!(morning.hour(), 8);
    assert_eq!(morning.minute(), 0);
    assert_eq!(morning.second(), 0);

    buttons[4].emit_clicked();
    assert!(matches!(command_rx.try_recv(), Ok(UiCommand::SetDnd(true))));
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn dnd_status_updates_text_and_visibility_together() {
    let label = gtk::Label::new(Some("stale"));

    update_dnd_status(&label, Utc::now().timestamp().saturating_add(60));
    assert!(label.is_visible());
    assert_eq!(label.text(), "· 1m");

    update_dnd_status(&label, Utc::now().timestamp().saturating_sub(1));
    assert!(!label.is_visible());
    assert!(label.text().is_empty());
}

#[gtk::test]
fn dropping_countdown_removes_its_live_source() {
    let callback_runs = Rc::new(Cell::new(0));
    let countdown = test_countdown(callback_runs.clone());

    drop(countdown);
    drain_main_context();

    assert_eq!(callback_runs.get(), 0);
}

fn test_countdown(callback_runs: Rc<Cell<u32>>) -> DndCountdown {
    let source = gtk::glib::idle_add_local(move || {
        callback_runs.set(callback_runs.get() + 1);
        gtk::glib::ControlFlow::Break
    });
    DndCountdown {
        source: Some(source),
        active: Rc::new(Cell::new(true)),
    }
}

fn drain_main_context() {
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}

fn attached_popover(toggle: &gtk::ToggleButton) -> gtk::Popover {
    let mut child = toggle.first_child();
    while let Some(widget) = child {
        if let Ok(popover) = widget.clone().downcast::<gtk::Popover>() {
            return popover;
        }
        child = widget.next_sibling();
    }
    panic!("DND toggle should own its duration popover");
}

fn menu_buttons(popover: &gtk::Popover) -> Vec<gtk::Button> {
    let choices = popover
        .child()
        .and_then(|child| child.downcast::<gtk::Box>().ok())
        .expect("DND popover should contain its choice box");
    let mut buttons = Vec::new();
    let mut child = choices.first_child();
    while let Some(widget) = child {
        if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
            buttons.push(button);
        }
        child = widget.next_sibling();
    }
    buttons
}

fn menu_separator(popover: &gtk::Popover) -> gtk::Separator {
    let choices = popover
        .child()
        .and_then(|child| child.downcast::<gtk::Box>().ok())
        .expect("DND popover should contain its choice box");
    let mut child = choices.first_child();
    while let Some(widget) = child {
        if let Ok(separator) = widget.clone().downcast::<gtk::Separator>() {
            return separator;
        }
        child = widget.next_sibling();
    }
    panic!("DND popover should separate the indefinite choice");
}
