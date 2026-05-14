//! Public facade for widget configuration types

#[path = "widgets/mod.rs"]
mod widgets;

pub use self::widgets::{
    CardWidgetConfig, NumericParseMode, SliderWidgetConfig, StatWidgetConfig, ToggleLayout,
    ToggleWidgetConfig, WidgetPluginConfig, WidgetsConfig,
};
