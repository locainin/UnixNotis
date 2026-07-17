use std::ops::Range;
use std::path::{Path, PathBuf};

use unixnotis_core::{parse_command, util, ExecutionMode, ParsedCommand};

use super::super::pathing::{format_relative_path, normalize_lexical_path};

pub fn resolve_command_path_token(config_dir: &Path, command: &str) -> Option<PathBuf> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = parse_command(trimmed).ok()?;
    let first = effective_program(&parsed)?;
    if !looks_like_path_token(first) {
        return None;
    }

    let expanded = PathBuf::from(util::expand_tilde(first).into_owned());
    if expanded.is_absolute() {
        return Some(expanded);
    }
    Some(config_dir.join(expanded))
}

pub fn collect_outside_env_path_tokens(config_dir: &Path, command: &str) -> Vec<(String, PathBuf)> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let normalized_root = normalize_lexical_path(config_dir);
    let Ok(parsed) = parse_command(trimmed) else {
        return Vec::new();
    };
    if parsed.execution_mode != ExecutionMode::Direct {
        // Shell operators change assignment scope, so executable-content review owns that case
        return Vec::new();
    }
    command_env_assignments(&parsed)
        .into_iter()
        .filter_map(|(name, value)| {
            if !env_name_needs_path_validation(name) {
                return None;
            }
            let outside_path = value
                .split(':')
                .filter(|part| !part.trim().is_empty())
                .filter_map(|part| resolve_env_path_value(config_dir, part))
                .find(|path| !normalize_lexical_path(path).starts_with(&normalized_root))?;
            Some((name.to_string(), outside_path))
        })
        .collect()
}

pub fn rewrite_command_to_config_relative(config_dir: &Path, command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed = parse_command(trimmed).ok()?;
    if parsed.execution_mode != ExecutionMode::Direct {
        // Rewriting shell syntax token-by-token could change operators or expansion behavior
        return None;
    }
    let first = effective_program(&parsed)?;
    if !is_host_specific_path_token(first) {
        return None;
    }

    let resolved_path = resolve_command_path_token(config_dir, trimmed)?;
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_path = normalize_lexical_path(&resolved_path);
    // Only paths that really live under the config root can be rewritten safely
    let relative_path = normalized_path.strip_prefix(&normalized_root).ok()?;
    let rewritten_first = format_relative_path(relative_path);
    if rewritten_first.is_empty() {
        return None;
    }

    // Re-quote parsed tokens so spaces survive without preserving ambiguous source quoting
    let mut words = parsed
        .env
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>();
    if parsed.program == "env" {
        let program_index = effective_program_index(&parsed)?;
        words.push(parsed.program);
        words.extend(parsed.args.into_iter().enumerate().map(|(index, token)| {
            if index == program_index {
                rewritten_first.clone()
            } else {
                token
            }
        }));
    } else {
        words.push(rewritten_first);
        words.extend(parsed.args);
    }
    Some(shell_words::join(words))
}

pub fn first_command_token(command: &str) -> Option<String> {
    // Returning the parsed program prevents quote characters from becoming path data
    let parsed = parse_command(command).ok()?;
    effective_program(&parsed).map(str::to_string)
}

fn command_env_assignments(parsed: &ParsedCommand) -> Vec<(&str, &str)> {
    let mut assignments = parsed
        .env
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<Vec<_>>();

    // `env NAME=value program` applies assignments to the eventual child too
    if parsed.program == "env" {
        if let Ok(layout) = env_command_layout(parsed) {
            assignments.extend(
                parsed.args[layout.assignment_range]
                    .iter()
                    .filter_map(|token| split_env_assignment(token)),
            );
        }
    }
    assignments
}

fn effective_program(parsed: &ParsedCommand) -> Option<&str> {
    if parsed.program != "env" {
        return Some(parsed.program.as_str());
    }

    // The env utility consumes leading assignments before spawning its real program
    effective_program_index(parsed).map(|index| parsed.args[index].as_str())
}

fn effective_program_index(parsed: &ParsedCommand) -> Option<usize> {
    env_command_layout(parsed).ok()?.program_index
}

pub(super) fn validate_env_command_layout(parsed: &ParsedCommand) -> Result<(), &'static str> {
    if parsed.program != "env" || parsed.execution_mode != ExecutionMode::Direct {
        return Ok(());
    }

    env_command_layout(parsed).map(|_| ())
}

struct EnvCommandLayout {
    assignment_range: Range<usize>,
    program_index: Option<usize>,
}

fn env_command_layout(parsed: &ParsedCommand) -> Result<EnvCommandLayout, &'static str> {
    let mut index = 0;
    let mut options_finished = false;

    while !options_finished {
        let Some(token) = parsed.args.get(index).map(String::as_str) else {
            break;
        };
        match token {
            "--" => {
                options_finished = true;
                index += 1;
            }
            "-"
            | "-i"
            | "-0"
            | "-v"
            | "--ignore-environment"
            | "--null"
            | "--debug"
            | "--list-signal-handling" => index += 1,
            "-u" | "--unset" | "-a" | "--argv0" => {
                // These options consume a value that must not be mistaken for the child program
                index = index
                    .checked_add(2)
                    .filter(|next| *next <= parsed.args.len())
                    .ok_or("env option is missing its required value")?;
            }
            "-C" | "--chdir" => {
                return Err("env working-directory options are not portable in preset commands");
            }
            "-S" | "--split-string" => {
                return Err("env split-string options are ambiguous in preset commands");
            }
            _ if token.starts_with("--chdir=") || token.starts_with("-C") => {
                return Err("env working-directory options are not portable in preset commands");
            }
            _ if token.starts_with("--split-string=") || token.starts_with("-S") => {
                return Err("env split-string options are ambiguous in preset commands");
            }
            _ if token.starts_with("--unset=")
                || token.starts_with("--argv0=")
                || attached_short_option(token, 'u')
                || attached_short_option(token, 'a') =>
            {
                index += 1;
            }
            _ if token == "--block-signal"
                || token.starts_with("--block-signal=")
                || token == "--default-signal"
                || token.starts_with("--default-signal=")
                || token == "--ignore-signal"
                || token.starts_with("--ignore-signal=") =>
            {
                index += 1;
            }
            _ if is_flag_only_short_cluster(token) => index += 1,
            _ if token.starts_with('-') => {
                return Err("env command contains an unsupported option form");
            }
            _ => break,
        }
    }

    let assignment_start = index;
    while parsed
        .args
        .get(index)
        .and_then(|token| split_env_assignment(token))
        .is_some()
    {
        index += 1;
    }

    Ok(EnvCommandLayout {
        assignment_range: assignment_start..index,
        program_index: (index < parsed.args.len()).then_some(index),
    })
}

fn split_env_assignment(token: &str) -> Option<(&str, &str)> {
    let (name, value) = token.split_once('=')?;
    let mut characters = name.chars();
    let first = characters.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }
    if characters.any(|character| !(character == '_' || character.is_ascii_alphanumeric())) {
        return None;
    }
    Some((name, value))
}

fn attached_short_option(token: &str, option: char) -> bool {
    let mut characters = token.chars();
    characters.next() == Some('-')
        && characters.next() == Some(option)
        && characters.next().is_some()
}

fn is_flag_only_short_cluster(token: &str) -> bool {
    token.strip_prefix('-').is_some_and(|flags| {
        !flags.is_empty() && flags.chars().all(|flag| matches!(flag, 'i' | '0' | 'v'))
    })
}

fn env_name_needs_path_validation(name: &str) -> bool {
    matches!(
        name,
        "PATH"
            | "HOME"
            | "LD_PRELOAD"
            | "LD_LIBRARY_PATH"
            | "LD_AUDIT"
            | "LD_CONFIG_FILE"
            | "PYTHONPATH"
            | "PYTHONHOME"
            | "PERL5LIB"
            | "RUBYLIB"
            | "NODE_PATH"
            | "GCONV_PATH"
            | "BASH_ENV"
            | "ENV"
            | "ZDOTDIR"
    )
}

fn resolve_env_path_value(config_dir: &Path, value: &str) -> Option<PathBuf> {
    let expanded = PathBuf::from(util::expand_tilde(value.trim()).into_owned());
    if expanded.is_absolute() {
        return Some(expanded);
    }
    if looks_like_path_token(value) {
        return Some(config_dir.join(expanded));
    }
    None
}

pub fn looks_like_path_token(token: &str) -> bool {
    token == "~"
        || token.starts_with("~/")
        || token.starts_with("./")
        || token.starts_with("../")
        || token.contains('/')
}

pub fn is_host_specific_path_token(token: &str) -> bool {
    token.starts_with('/') || token == "~" || token.starts_with("~/")
}
