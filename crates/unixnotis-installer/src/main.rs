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
mod detect;
mod events;
// Keep installer entrypoint lean by delegating to modules stored under src/main/.
#[path = "main/main_actions.rs"]
mod main_actions;
#[path = "main/main_flow.rs"]
mod main_flow;
#[path = "main/main_handlers.rs"]
mod main_handlers;
mod model;
mod paths;
mod terminal;
mod ui;

use anyhow::Result;
use std::path::PathBuf;

use crate::main_actions::run_trial;
use crate::main_flow::run_app;
use crate::terminal::TerminalGuard;

fn main() -> Result<()> {
    let mut app = app::App::new();
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
