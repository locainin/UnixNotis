//! Shared theme token contract for legacy and modern GTK CSS paths

mod layout;
mod legacy;
mod model;
mod modern;
mod palette;

pub use legacy::build_legacy_theme_color_overrides;
pub use model::{theme_card_style_values, ThemeCardStyleValues};
pub use modern::build_modern_theme_custom_properties;

#[cfg(test)]
mod tests;
