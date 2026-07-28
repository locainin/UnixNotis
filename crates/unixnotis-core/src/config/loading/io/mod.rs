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

#[cfg(test)]
mod tests;
