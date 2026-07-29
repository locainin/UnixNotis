//! Theme compatibility notice action wiring

use async_channel::TrySendError;
use gtk::prelude::*;

use crate::control::UiEvent;
use crate::ui::panel::notice::ReloadNoticeWidgets;

pub(in crate::ui) fn connect_notice_actions(
    notice: &ReloadNoticeWidgets,
    event_tx: async_channel::Sender<UiEvent>,
) {
    let stock_tx = event_tx.clone();
    notice.use_stock_button.connect_clicked(move |_| {
        send_action(&stock_tx, UiEvent::UseStockTheme);
    });

    notice.open_theme_folder_button.connect_clicked(move |_| {
        send_action(&event_tx, UiEvent::OpenThemeFolder);
    });
}

fn send_action(event_tx: &async_channel::Sender<UiEvent>, event: UiEvent) {
    match event_tx.try_send(event) {
        Ok(()) => {}
        Err(TrySendError::Full(event)) => {
            // Explicit choices wait for queue capacity instead of disappearing under load
            let event_tx = event_tx.clone();
            gtk::glib::MainContext::default().spawn_local(async move {
                let _result = event_tx.send(event).await;
            });
        }
        Err(TrySendError::Closed(_event)) => {
            // Shutdown already owns the UI when the receiver is gone
        }
    }
}
