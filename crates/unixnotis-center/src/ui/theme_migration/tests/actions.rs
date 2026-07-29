//! Migration notice action wiring tests

use gtk::prelude::*;

use super::super::connect_notice_actions;
use crate::control::UiEvent;
use crate::ui::panel::notice::build_reload_notice;

#[gtk::test]
fn migration_buttons_emit_three_distinct_policy_events() {
    let notice = build_reload_notice();
    let (event_tx, event_rx) = async_channel::bounded(3);
    connect_notice_actions(&notice, event_tx);

    notice.preview_button.emit_clicked();
    notice.apply_button.emit_clicked();
    notice.keep_button.emit_clicked();

    assert!(matches!(
        event_rx.try_recv().expect("Preview should emit an event"),
        UiEvent::ThemeMigrationPreview
    ));
    assert!(matches!(
        event_rx.try_recv().expect("Apply should emit an event"),
        UiEvent::ThemeMigrationApply
    ));
    assert!(matches!(
        event_rx
            .try_recv()
            .expect("Keep Current should emit an event"),
        UiEvent::ThemeMigrationKeepCurrent
    ));
}
