//! Synchronous and asynchronous command construction for daemon helpers

use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as TokioCommand;

pub fn command(program: &str) -> std::io::Result<Command> {
    // Resolve first so std::process never searches inherited PATH
    Ok(Command::new(program_path(program)?))
}

pub fn tokio_command(program: &str) -> std::io::Result<TokioCommand> {
    // Tokio receives the same resolved path as synchronous process launches
    Ok(TokioCommand::new(program_path(program)?))
}

pub fn program_path(program: &str) -> std::io::Result<PathBuf> {
    // Missing trusted tools are reported before any process state is created
    super::routing::trusted_program_path(program).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program} not found in trusted system tool directories"),
        )
    })
}
