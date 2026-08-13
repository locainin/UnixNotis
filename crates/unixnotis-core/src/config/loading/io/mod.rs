//! Configuration filesystem operations

mod error;
mod load;
mod paths;
mod script_migrations;
mod scripts;
mod theme_contract;

pub use error::ConfigError;
pub use load::MAX_CONFIG_BYTES;
pub use paths::ThemePaths;
pub use theme_contract::{
    ThemeContractState, ThemeIncompatibility, ThemeManifest, THEME_API_VERSION,
};

#[cfg(test)]
mod tests;
