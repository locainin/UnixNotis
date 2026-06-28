//! UnixNotis installer entrypoint with a ratatui-driven flow.

#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

mod actions;
mod app;
mod checks;
mod cli;
mod detect;
mod events;
// Keep installer entrypoint lean by delegating to modules stored under src/main/.
#[path = "main/action_workflow.rs"]
mod action_workflow;
#[path = "main/main_flow.rs"]
mod main_flow;
#[path = "main/main_handlers.rs"]
mod main_handlers;
#[cfg(test)]
#[path = "main/tests.rs"]
mod main_tests;
mod model;
#[path = "paths/index.rs"]
mod paths;
mod service_manager;
mod terminal;
#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
#[path = "main/trial/index.rs"]
mod trial;
mod ui;

use anyhow::Result;
use std::path::PathBuf;

use crate::cli::CliAction;
use crate::main_flow::run_app;
use crate::terminal::TerminalGuard;
use crate::trial::run_trial;

fn main() -> Result<()> {
    let cli = match cli::parse_env_args()? {
        CliAction::Run(args) => args,
        CliAction::Help => {
            print!("{}", cli::usage());
            return Ok(());
        }
    };
    let mut app = app::App::new(cli.service_manager);
    let mut terminal_guard = TerminalGuard::new()?;
    let exit_action = run_app(&mut terminal_guard, &mut app);
    terminal_guard.restore()?;

    match exit_action {
        Ok(ExitAction::None) => Ok(()),
        Ok(ExitAction::RunTrial { repo_root }) => run_trial(repo_root),
        Err(err) => Err(err),
    }
}

pub(crate) enum ExitAction {
    None,
    RunTrial { repo_root: PathBuf },
}
