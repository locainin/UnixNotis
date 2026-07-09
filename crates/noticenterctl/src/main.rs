#![allow(
    clippy::nursery,
    clippy::pedantic,
    reason = "pedantic and nursery cleanup is tracked incrementally across existing code"
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
