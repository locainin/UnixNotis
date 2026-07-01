//! Public widget configuration exports

#[path = "widgets/mod.rs"]
mod widgets;

pub use self::widgets::{
    CardLayout, CardWidgetConfig, NumericParseMode, SliderWidgetConfig, StatWidgetConfig,
    ToggleLayout, ToggleWidgetConfig, WidgetPluginConfig, WidgetsConfig,
};
