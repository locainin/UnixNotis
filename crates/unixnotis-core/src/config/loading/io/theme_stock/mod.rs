//! Explicit, non-destructive stock theme migration

pub(in crate::config::loading::io) mod files;
mod migration;
mod model;
mod staging;

pub use migration::{
    apply_stock_theme_migration, detect_stock_theme_migration, keep_current_stock_theme,
};
pub use model::{StockThemeApplyReport, StockThemeMigration};
pub(in crate::config::loading::io) use staging::stage_current_stock_themes;

const MAX_STOCK_THEME_BYTES: u64 = 1_048_576;
const MAX_STOCK_PATH_COLLISIONS: u8 = 8;

#[cfg(test)]
mod tests;
