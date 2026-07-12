//! Popup application entrypoint and top-level module wiring

#![allow(
    clippy::nursery,
    clippy::pedantic,
    reason = "pedantic and nursery cleanup is tracked incrementally across existing code"
)]

use anyhow::Result;
use clap::Parser;

#[path = "app/mod.rs"]
mod app;
mod dbus;
#[cfg(test)]
mod test_support;
mod ui;

fn main() -> Result<()> {
    let args = app::Args::parse();
    app::run(args)
}
