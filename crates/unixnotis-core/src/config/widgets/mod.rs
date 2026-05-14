//! Widget config types split by widget family

mod cards;
mod plugin;
mod root;
mod sliders;
mod stats;
mod toggles;

pub use self::cards::CardWidgetConfig;
pub use self::plugin::WidgetPluginConfig;
pub use self::root::WidgetsConfig;
pub use self::sliders::{NumericParseMode, SliderWidgetConfig};
pub use self::stats::StatWidgetConfig;
pub use self::toggles::{ToggleLayout, ToggleWidgetConfig};
