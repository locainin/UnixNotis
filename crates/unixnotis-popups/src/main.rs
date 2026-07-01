//! Popup application entrypoint and top-level module wiring

#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
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
