//! Local theme management commands

mod export;

use anyhow::Result;

use crate::cli::ThemeCommand;

pub fn run(command: ThemeCommand) -> Result<()> {
    match command {
        ThemeCommand::ExportStock { output } => export::run(output),
    }
}

#[cfg(test)]
mod tests;
