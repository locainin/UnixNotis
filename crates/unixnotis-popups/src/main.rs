//! Popup application entrypoint and top-level module wiring

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::default_trait_access,
    clippy::future_not_send,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::struct_field_names,
    clippy::too_many_lines,
    reason = "reviewed GTK pixel math, main-thread futures, and popup state boundaries retain stable compositor-facing behavior"
)]

use anyhow::Result;
use clap::Parser;

#[path = "app/mod.rs"]
mod app;
mod dbus;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
mod ui;

fn main() -> Result<()> {
    let args = app::Args::parse();
    app::run(args)
}
