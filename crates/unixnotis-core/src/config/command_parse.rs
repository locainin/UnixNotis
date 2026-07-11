//! Shared widget command parsing policy

use thiserror::Error;

use crate::util::SHELL_META_CHARS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionMode {
    // Direct commands are spawned without a shell
    Direct,
    // Shell commands retain syntax that must be interpreted by `sh -c`
    Shell,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedCommand {
    // Leading assignments apply only to the spawned command
    pub env: Vec<(String, String)>,
    // Program and arguments are unquoted exactly once by the shared parser
    pub program: String,
    pub args: Vec<String>,
    pub execution_mode: ExecutionMode,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CommandParseError {
    #[error("command is empty")]
    Empty,
    #[error("command contains malformed shell quoting: {0}")]
    Malformed(String),
    #[error("command contains environment assignments but no program")]
    MissingProgram,
}

pub fn parse_command(command: &str) -> Result<ParsedCommand, CommandParseError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(CommandParseError::Empty);
    }

    // One parser owns quote removal for both runtime execution and preset review
    let parts = shell_words::split(trimmed)
        .map_err(|error| CommandParseError::Malformed(error.to_string()))?;
    let (env, remaining) = split_leading_env_assignments(parts);
    let mut remaining = remaining.into_iter();
    let program = remaining.next().ok_or(CommandParseError::MissingProgram)?;
    let args = remaining.collect();

    // Shell syntax stays visible in the mode even though tokens are available for review
    let execution_mode = if requires_shell(trimmed) {
        ExecutionMode::Shell
    } else {
        ExecutionMode::Direct
    };

    Ok(ParsedCommand {
        env,
        program,
        args,
        execution_mode,
    })
}

fn split_leading_env_assignments(parts: Vec<String>) -> (Vec<(String, String)>, Vec<String>) {
    let mut env = Vec::new();
    let mut index = 0;

    // Assignment scanning ends at the first token that is not a valid shell name
    while let Some(token) = parts.get(index) {
        let Some((name, value)) = split_env_assignment(token) else {
            break;
        };
        env.push((name.to_string(), value.to_string()));
        index += 1;
    }

    (env, parts[index..].to_vec())
}

fn split_env_assignment(token: &str) -> Option<(&str, &str)> {
    let (name, value) = token.split_once('=')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }
    if chars.any(|character| !(character == '_' || character.is_ascii_alphanumeric())) {
        return None;
    }
    Some((name, value))
}

fn requires_shell(command: &str) -> bool {
    command.chars().any(|character| {
        SHELL_META_CHARS.contains(&character)
            || character == '~'
            || character == '\n'
            || character == '\r'
    })
}

#[cfg(test)]
#[path = "tests/command_parse.rs"]
mod tests;
