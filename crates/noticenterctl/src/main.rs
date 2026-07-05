#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

mod app;
mod cli;
mod css_check;
mod dbus;
mod debug_logs;
mod output;
mod preset;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
