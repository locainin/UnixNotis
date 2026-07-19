use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use super::{connect_clear_button, connect_dnd_button};
use crate::control::UiCommand;

#[gtk::test]
fn clear_button_sends_once_while_click_guard_is_active() {
    let button = gtk::Button::new();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(2);
    connect_clear_button(&button, command_tx);

    button.emit_clicked();
    button.emit_clicked();

    assert!(matches!(command_rx.try_recv(), Ok(UiCommand::ClearAll)));
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn dnd_toggle_waits_for_daemon_state_before_changing_visual_state() {
    let button = gtk::ToggleButton::new();
    let guard = Rc::new(Cell::new(false));
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(2);
    connect_dnd_button(&button, guard, command_tx);

    button.set_active(true);

    assert!(!button.is_active());
    assert!(matches!(command_rx.try_recv(), Ok(UiCommand::SetDnd(true))));
    assert!(command_rx.try_recv().is_err());
}
