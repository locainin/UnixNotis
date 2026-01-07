//! Embedded default theme fragments for UnixNotis.

pub const DEFAULT_BASE_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/base.css"));

pub const DEFAULT_PANEL_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/panel.css"));

pub const DEFAULT_POPUP_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/popup.css"));

pub const DEFAULT_WIDGETS_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/widgets.css"));
