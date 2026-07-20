//! Panel configuration types, ordering, and action metadata

mod actions;
mod config;
mod dnd;
mod empty;
mod metadata;
mod sections;

pub use self::actions::{
    default_panel_action_order, PanelActionConfig, PanelActionId, PanelClearButtonPlacement,
};
pub use self::config::PanelConfig;
pub use self::dnd::{
    default_dnd_menu_choices, default_dnd_menu_triggers, DndMenuChoice, DndMenuTrigger,
};
pub use self::empty::EmptyStateAlignment;
pub use self::metadata::NotificationMetadataConfig;
pub use self::sections::{
    default_panel_section_order, default_panel_widget_order, PanelSection, PanelWidgetSection,
};

#[cfg(test)]
mod tests;
