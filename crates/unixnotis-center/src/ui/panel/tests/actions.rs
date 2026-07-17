use gtk::prelude::*;

use super::connect_clear_button;
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
