//! Process construction after trusted tool resolution

use std::path::PathBuf;
use std::process::Command;
use unixnotis_core::CommandSpec;

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

pub fn command_from_spec(spec: &CommandSpec) -> std::io::Result<Command> {
    let (program, args, env) = direct_parts(spec)?;
    let mut command = command(program)?;
    command.args(args).envs(env);
    Ok(command)
}

pub fn tokio_command_from_spec(spec: &CommandSpec) -> std::io::Result<tokio::process::Command> {
    let (program, args, env) = direct_parts(spec)?;
    let path = trusted_program_path(program).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program} not found in trusted system tool directories"),
        )
    })?;
    let mut command = tokio::process::Command::new(path);
    command.args(args).envs(env);
    Ok(command)
}

fn direct_parts(
    spec: &CommandSpec,
) -> std::io::Result<(
    &str,
    &[std::ffi::OsString],
    &std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString>,
)> {
    let CommandSpec::Direct { program, args, env } = spec else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "trusted system tool commands must use direct mode",
        ));
    };
    let program = program.to_str().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "trusted system tool program is not UTF-8",
        )
    })?;
    Ok((program, args, env))
}

pub fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // Routing differs only in tests, while validation stays in the shared lookup layer
    super::routing::trusted_program_path(program)
}
