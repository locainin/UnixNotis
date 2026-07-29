//! Configuration filesystem operations

mod error;
mod load;
mod paths;
mod script_migrations;
mod scripts;
mod theme_files;
mod theme_stock;
mod write;

pub use error::ConfigError;
pub use load::MAX_CONFIG_BYTES;
pub use paths::ThemePaths;
pub use theme_stock::{
    apply_stock_theme_migration, detect_stock_theme_migration, keep_current_stock_theme,
    StockThemeApplyReport, StockThemeMigration,
};

#[cfg(test)]
mod tests;
