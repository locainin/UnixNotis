//! One-way migration from legacy shell-shaped command strings

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

use super::CommandSpec;

const VALUE_PLACEHOLDER: &str = "{value}";

#[derive(Debug, Error, Eq, PartialEq)]
pub enum LegacyCommandError {
    #[error("command is empty")]
    Empty,
    #[error("command contains malformed shell quoting: {0}")]
    Malformed(String),
    #[error("command contains environment assignments but no program")]
    MissingProgram,
}

/// Convert one legacy command string into an explicit direct or shell specification
///
/// # Errors
///
/// Returns an error when the legacy command is empty, malformed, or has no program
pub fn parse_legacy_command(command: &str) -> Result<CommandSpec, LegacyCommandError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(LegacyCommandError::Empty);
    }

    let parts = shell_words::split(trimmed)
        .map_err(|error| LegacyCommandError::Malformed(error.to_string()))?;
    // Shell operators are detected before quote removal so literal punctuation stays direct
    if contains_shell_syntax(trimmed) {
        return Ok(CommandSpec::shell(trimmed));
    }
    let (env, remaining) = split_leading_env_assignments(parts);
    let mut remaining = remaining.into_iter();
    let program = remaining.next().ok_or(LegacyCommandError::MissingProgram)?;

    let spec = CommandSpec::Direct {
        program: PathBuf::from(program),
        args: remaining.map(OsString::from).collect(),
        env,
    };
    if let Some(script) = exact_shell_c_script(&spec) {
        return Ok(CommandSpec::shell(script));
    }
    Ok(spec)
}

fn exact_shell_c_script(spec: &CommandSpec) -> Option<&str> {
    let CommandSpec::Direct { args, env, .. } = spec else {
        return None;
    };
    // Environment prefixes and extra operands change shell wrapper semantics
    if !env.is_empty() || !spec.uses_shell_command_string() {
        return None;
    }
    let [flag, script] = args.as_slice() else {
        return None;
    };
    if flag != "-c" {
        return None;
    }
    script.to_str()
}

fn split_leading_env_assignments(
    mut parts: Vec<String>,
) -> (BTreeMap<OsString, OsString>, Vec<String>) {
    let assignment_count = parts
        .iter()
        .take_while(|token| split_env_assignment(token).is_some())
        .count();
    let remaining = parts.split_off(assignment_count);
    let env = parts
        .iter()
        .filter_map(|token| split_env_assignment(token))
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect();
    (env, remaining)
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

fn contains_shell_syntax(command: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    let mut token_start = true;
    let mut chars = command.char_indices();

    while let Some((index, character)) = chars.next() {
        if escaped {
            escaped = false;
            token_start = false;
            continue;
        }

        match quote {
            Some('\'') => {
                if character == '\'' {
                    quote = None;
                }
                continue;
            }
            Some('"') => {
                match character {
                    '"' => quote = None,
                    '\\' => escaped = true,
                    '$' | '`' => return true,
                    _ => {}
                }
                continue;
            }
            Some(_) => unreachable!("legacy scanner stores only shell quote characters"),
            None => {}
        }

        match character {
            '\'' | '"' => {
                quote = Some(character);
                token_start = false;
            }
            '\\' => {
                escaped = true;
                token_start = false;
            }
            ' ' | '\t' => token_start = true,
            '#' | '!' if token_start => return true,
            '{' if command[index..].starts_with(VALUE_PLACEHOLDER) => {
                // Skip the rest of the known runtime placeholder as literal direct data
                for _ in 1..VALUE_PLACEHOLDER.len() {
                    let _ = chars.next();
                }
                token_start = false;
            }
            '\n' | '\r' | '|' | '&' | ';' | '<' | '>' | '$' | '`' | '(' | ')' | '[' | ']' | '*'
            | '?' | '~' | '{' | '}' => return true,
            _ => token_start = false,
        }
    }

    // shell_words validates these states before this classifier runs
    debug_assert!(
        quote.is_none() && !escaped,
        "validated legacy command must finish outside quoted or escaped input"
    );
    false
}
