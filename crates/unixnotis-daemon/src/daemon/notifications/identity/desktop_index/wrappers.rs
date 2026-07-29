//! Generic launch-wrapper normalization before executable identity is resolved

use super::model::LaunchWrapper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NormalizedLaunchCommand {
    pub(super) executable: String,
    pub(super) arguments: Vec<String>,
    pub(super) environment: Vec<(Vec<u8>, Vec<u8>)>,
    pub(super) wrappers: Vec<LaunchWrapper>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum ExecParseError {
    EmptyCommand,
    MalformedEnvCommand,
    MissingWrappedCommand,
    UnsupportedWrapper,
}

pub(super) fn normalize_launch_command(
    tokens: Vec<String>,
) -> Result<NormalizedLaunchCommand, ExecParseError> {
    if tokens.is_empty() {
        return Err(ExecParseError::EmptyCommand);
    }

    let mut current = tokens;
    let mut environment = Vec::new();
    let mut wrappers = Vec::new();
    while let Some(prefix) = unwrap_env(&current)? {
        // Each wrapper consumes a strict prefix and leaves one complete command
        environment.extend(prefix.environment);
        wrappers.push(prefix.wrapper);
        current = prefix.remaining_command;
    }

    let mut current = current.into_iter();
    let executable = current.next().ok_or(ExecParseError::EmptyCommand)?;
    Ok(NormalizedLaunchCommand {
        executable,
        arguments: current.collect(),
        environment,
        wrappers,
    })
}

struct NormalizedPrefix {
    remaining_command: Vec<String>,
    environment: Vec<(Vec<u8>, Vec<u8>)>,
    wrapper: LaunchWrapper,
}

fn unwrap_env(tokens: &[String]) -> Result<Option<NormalizedPrefix>, ExecParseError> {
    let Some(first) = tokens.first() else {
        return Err(ExecParseError::EmptyCommand);
    };
    if first != "env" && first != "/usr/bin/env" {
        return Ok(None);
    }

    let mut index = 1;
    let mut environment = Vec::new();
    while let Some(token) = tokens.get(index) {
        if token == "--" {
            advance_index(&mut index, 1)?;
            break;
        }
        if token == "-i" || token == "--ignore-environment" {
            advance_index(&mut index, 1)?;
            continue;
        }
        if token == "-u" {
            if tokens.get(index + 1).is_none() {
                return Err(ExecParseError::MalformedEnvCommand);
            }
            advance_index(&mut index, 2)?;
            continue;
        }
        if token.starts_with("--unset=") {
            advance_index(&mut index, 1)?;
            continue;
        }
        if token.starts_with('-') {
            // Options such as -S change tokenization and need a dedicated safe parser
            return Err(ExecParseError::UnsupportedWrapper);
        }
        if let Some((name, value)) = parse_environment_assignment(token) {
            environment.push((name.as_bytes().to_vec(), value.as_bytes().to_vec()));
            advance_index(&mut index, 1)?;
            continue;
        }
        break;
    }

    if index >= tokens.len() {
        return Err(ExecParseError::MissingWrappedCommand);
    }
    Ok(Some(NormalizedPrefix {
        remaining_command: tokens[index..].to_vec(),
        environment,
        wrapper: LaunchWrapper::Env,
    }))
}

fn advance_index(index: &mut usize, amount: usize) -> Result<(), ExecParseError> {
    // Checked progress prevents malformed input from wrapping the parser cursor
    *index = index
        .checked_add(amount)
        .ok_or(ExecParseError::MalformedEnvCommand)?;
    Ok(())
}

fn parse_environment_assignment(value: &str) -> Option<(&str, &str)> {
    let (name, assigned) = value.split_once('=')?;
    let mut characters = name.chars();
    let first = characters.next()?;
    if !(first == '_' || first.is_ascii_alphabetic())
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return None;
    }
    Some((name, assigned))
}

#[cfg(test)]
#[path = "tests/wrappers.rs"]
mod tests;
