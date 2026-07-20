#![expect(
    clippy::assigning_clones,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::format_push_string,
    clippy::future_not_send,
    clippy::match_same_arms,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unused_peekable,
    clippy::unused_self,
    reason = "reviewed CLI parsing, report assembly, and async D-Bus boundaries retain explicit forms for stable diagnostics and command behavior"
)]

mod app;
mod cli;
mod config_path;
mod css_check;
mod dbus;
mod debug_logs;
mod doctor;
mod output;
mod preset;
mod session_environment;
mod system_tools;

use std::process::ExitCode;

#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;

fn main() -> ExitCode {
    match app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = output::write_stderr(&output::format_cli_error(&error));
            ExitCode::FAILURE
        }
    }
}
