//! Installer command construction and availability probes

use std::path::PathBuf;
use std::process::Command;

pub fn command(program: &str) -> std::io::Result<Command> {
    // Resolve before construction so installer probes cannot inherit a hostile PATH entry
    Ok(Command::new(program_path(program)?))
}

pub fn program_path(program: &str) -> std::io::Result<PathBuf> {
    // Callers receive one consistent missing-tool error without starting a child
    super::routing::trusted_program_path(program).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program} not found in trusted system tool directories"),
        )
    })
}

pub fn program_exists(program: &str) -> bool {
    // Availability uses the same policy as actual command construction
    super::routing::trusted_program_path(program).is_some()
}
