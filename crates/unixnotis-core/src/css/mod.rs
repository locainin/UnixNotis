//! Shared GTK CSS feature gates, theme tokens, and class hooks

// Keep CSS support rules in one place so UI, installer, and checker stay aligned
#[path = "features.rs"]
pub mod features;
// Shared class names stop theme hooks from drifting between widgets
pub mod hooks;
// Token builders keep legacy colors and newer custom properties in sync
#[path = "tokens.rs"]
pub mod tokens;

pub use self::features::{
    gtk_css_features_for_version, gtk_css_features_from_version_string, GtkCssFeatures,
    GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL,
};
pub use self::tokens::{
    build_legacy_theme_color_overrides, build_modern_theme_custom_properties,
    theme_card_style_values, ThemeCardStyleValues,
};
