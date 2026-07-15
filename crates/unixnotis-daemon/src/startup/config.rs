//! Configuration selection for daemon startup

use anyhow::{Context, Result};
use unixnotis_core::Config;

use crate::cli::Args;

pub fn load_config(args: &Args) -> Result<Config> {
    match args.config.as_ref() {
        Some(path) => Config::load_from_path(path).context("read config from path"),
        None => Config::load_default().context("read default config"),
    }
}
