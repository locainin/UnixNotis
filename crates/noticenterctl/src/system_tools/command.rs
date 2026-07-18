//! Process construction after trusted tool resolution

use std::path::PathBuf;
use std::process::Command;

pub fn command(program: &str) -> std::io::Result<Command> {
    // Resolve before construction so inherited PATH never selects the executable
    let path = trusted_program_path(program).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program} not found in trusted system tool directories"),
        )
    })?;
    // Arguments remain the caller's responsibility and never pass through a shell
    Ok(Command::new(path))
}

pub fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // Routing differs only in tests, while validation stays in the shared lookup layer
    super::routing::trusted_program_path(program)
}
