//! Communication popup with quiet application identity and message-first hierarchy

use unixnotis_core::NotificationView;

use super::layout::{build_popup_grid, PopupLayout};
use super::RenderedPopup;
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

pub(super) fn build_communication_popup(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
) -> RenderedPopup {
    build_popup_grid(
        state,
        notification,
        view,
        PopupLayout {
            css_class: "unixnotis-popup-communication-content",
            body_lines: 5,
            show_reply_note: true,
        },
    )
}
