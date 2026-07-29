//! Compact utility popup for device, transfer, clipboard, and generic events

use unixnotis_core::NotificationView;

use super::layout::{build_popup_grid, PopupLayout};
use super::RenderedPopup;
use crate::ui::entry::presentation::PopupEntryViewModel;
use crate::ui::UiState;

pub(super) fn build_utility_popup(
    state: &mut UiState,
    notification: &NotificationView,
    view: &PopupEntryViewModel,
) -> RenderedPopup {
    build_popup_grid(
        state,
        notification,
        view,
        PopupLayout {
            css_class: "unixnotis-popup-utility-content",
            body_lines: 2,
            show_reply_note: false,
        },
    )
}
