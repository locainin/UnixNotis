#![expect(
    clippy::assigning_clones,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::format_push_string,
    clippy::future_not_send,
    clippy::items_after_statements,
    clippy::match_same_arms,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unused_peekable,
    clippy::unused_self,
    clippy::useless_let_if_seq,
    reason = "reviewed CLI parsing, report assembly, and async D-Bus boundaries retain explicit forms for stable diagnostics and command behavior"
)]

mod app;
mod cli;
mod css_check;
mod dbus;
mod debug_logs;
mod output;
mod preset;
mod system_tools;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
