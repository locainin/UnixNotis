//! Shared layout primitives and popup configuration

mod common;
mod popup;

pub use self::common::{
    Anchor, Margins, PanelKeyboardInteractivity, PANEL_HEIGHT_PERCENT_DEFAULT,
    PANEL_RUNTIME_WIDTH_MIN,
};
pub use self::popup::PopupConfig;
