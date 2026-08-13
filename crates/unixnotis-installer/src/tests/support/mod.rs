//! Shared installer test helpers for environment and filesystem fixtures

use unixnotis_core::CURRENT_CONFIG_VERSION;

pub mod env;
pub mod fs;
mod paths;

pub fn current_config_text(contents: &str) -> String {
    format!("config_version = {CURRENT_CONFIG_VERSION}\n{contents}")
}

#[cfg(test)]
mod tests;
