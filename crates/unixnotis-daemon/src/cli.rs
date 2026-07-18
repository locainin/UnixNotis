//! Daemon command-line contract

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
pub struct Args {
    /// Path to config.toml
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Run in trial mode and replace any existing daemon
    #[arg(long)]
    pub trial: bool,
    /// Restore strategy after trial mode ends
    #[arg(long, value_enum, default_value_t = RestoreStrategy::Auto)]
    pub restore: RestoreStrategy,
    /// Skip confirmation prompt in trial mode
    #[arg(long)]
    pub yes: bool,
    /// Time to wait for another daemon to re-acquire after release in milliseconds
    #[arg(long, default_value_t = 2000)]
    pub restore_wait_ms: u64,
    /// Validate configuration and exit
    #[arg(long)]
    pub check: bool,
    /// Exit after running for the requested number of seconds
    #[arg(long)]
    pub run_seconds: Option<u64>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum RestoreStrategy {
    Auto,
    None,
    Systemd,
    Process,
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
