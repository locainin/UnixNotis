//! `UnixNotis` installer entrypoint with a ratatui-driven flow

#![expect(
    clippy::collapsible_else_if,
    clippy::items_after_statements,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::redundant_else,
    clippy::ref_option,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    reason = "reviewed installer state-machine, backend, and TUI boundaries keep explicit control flow for auditable lifecycle behavior"
)]

mod actions;
mod app;
mod checks;
mod cli;
mod detect;
mod managed_binaries;
mod model;
mod paths;
mod release;
mod safe_write;
mod service_manager;
mod system_tools;
mod terminal;
#[cfg(test)]
#[path = "tests/support/mod.rs"]
mod test_support;
mod trial;
mod ui;

use anyhow::Result;

use crate::app::runtime::run_app;
use crate::app::{App, ExitAction};
use crate::cli::CliAction;
use crate::terminal::TerminalGuard;
use crate::trial::run_trial;

fn main() -> Result<()> {
    let cli = match cli::parse_env_args()? {
        CliAction::Run(args) => args,
        CliAction::Help => {
            print!("{}", cli::usage());
            return Ok(());
        }
        CliAction::Version => {
            println!("{}", cli::version());
            return Ok(());
        }
    };
    let mut app = App::new(cli.service_manager);
    let mut terminal_guard = TerminalGuard::new()?;
    let exit_action = run_app(&mut terminal_guard, &mut app);
    terminal_guard.restore()?;

    match exit_action {
        Ok(ExitAction::None) => Ok(()),
        Ok(ExitAction::RunTrial { repo_root }) => run_trial(repo_root),
        Err(err) => Err(err),
    }
}
