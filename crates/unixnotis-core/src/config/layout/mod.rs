//! Shared layout primitives and popup configuration

mod common;
mod popup;

#[cfg(test)]
mod tests;

pub use self::common::{
    Anchor, Margins, PanelKeyboardInteractivity, PANEL_HEIGHT_PERCENT_DEFAULT,
    PANEL_RUNTIME_WIDTH_MIN,
};
pub use self::popup::{PopupConfig, MAX_POPUP_TIMEOUT_MS};
