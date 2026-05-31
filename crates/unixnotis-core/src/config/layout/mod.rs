//! Layout-related configuration types for the panel and popups

mod common;
mod panel;
mod panel_actions;
mod panel_sections;
mod popup;

pub use self::common::{
    Anchor, Margins, PanelKeyboardInteractivity, PANEL_HEIGHT_PERCENT_DEFAULT,
    PANEL_RUNTIME_WIDTH_MIN,
};
pub use self::panel::PanelConfig;
pub use self::panel_actions::{
    default_panel_action_order, PanelActionConfig, PanelActionId, PanelClearButtonPlacement,
};
pub use self::panel_sections::{
    default_panel_section_order, default_panel_widget_order, PanelSection, PanelWidgetSection,
};
pub use self::popup::PopupConfig;
