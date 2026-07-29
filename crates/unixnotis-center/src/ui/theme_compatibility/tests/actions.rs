use gtk::prelude::*;

use super::super::connect_notice_actions;
use crate::control::UiEvent;
use crate::ui::panel::notice::build_reload_notice;

#[gtk::test]
fn compatibility_buttons_emit_stock_and_folder_events() {
    let notice = build_reload_notice();
    let (event_tx, event_rx) = async_channel::bounded(2);
    connect_notice_actions(&notice, event_tx);

    notice.use_stock_button.emit_clicked();
    notice.open_theme_folder_button.emit_clicked();

    assert!(matches!(
        event_rx.try_recv().expect("stock action should emit"),
        UiEvent::UseStockTheme
    ));
    assert!(matches!(
        event_rx.try_recv().expect("folder action should emit"),
        UiEvent::OpenThemeFolder
    ));
}
