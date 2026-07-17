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
    command_env_assignments(&parsed)
        .into_iter()
        .filter_map(|(name, value)| {
            let components = env_path_components(name, value).ok().flatten()?;
            let outside_path = components
                .into_iter()
                .map(|part| resolve_env_path_value(config_dir, part))
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
    // Shell-mode parsing is conservative because unsafe assignments must fail closed
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

pub(super) fn validate_env_path_semantics(parsed: &ParsedCommand) -> Result<(), &'static str> {
    for (name, value) in command_env_assignments(parsed) {
        let _ = env_path_components(name, value)?;
    }
    Ok(())
}

struct EnvCommandLayout {
    assignment_range: Range<usize>,
    program_index: Option<usize>,
}

enum EnvOptionStep {
    Continue(usize),
    Finish(usize),
    Stop,
}

fn env_command_layout(parsed: &ParsedCommand) -> Result<EnvCommandLayout, &'static str> {
    let mut option_count = 0usize;
    let mut remaining = parsed.args.as_slice();
    loop {
        match env_option_step(remaining)? {
            EnvOptionStep::Continue(width) => {
                option_count = option_count
                    .checked_add(width)
                    .ok_or("env option count overflowed")?;
                remaining = &remaining[width..];
            }
            EnvOptionStep::Finish(width) => {
                option_count = option_count
                    .checked_add(width)
                    .ok_or("env option count overflowed")?;
                remaining = &remaining[width..];
                break;
            }
            EnvOptionStep::Stop => break,
        }
    }

    // Counting a slice cannot get stuck when a malformed option shifts the child position
    let assignment_count = remaining
        .iter()
        .take_while(|token| split_env_assignment(token).is_some())
        .count();
    let program_index = option_count
        .checked_add(assignment_count)
        .ok_or("env argument count overflowed")?;

    Ok(EnvCommandLayout {
        assignment_range: option_count..program_index,
        program_index: (program_index < parsed.args.len()).then_some(program_index),
    })
}

fn env_option_step(arguments: &[String]) -> Result<EnvOptionStep, &'static str> {
    let Some(token) = arguments.first().map(String::as_str) else {
        return Ok(EnvOptionStep::Stop);
    };
    if token == "--" {
        return Ok(EnvOptionStep::Finish(1));
    }
    if is_flag_only_env_option(token)
        || is_attached_value_option(token)
        || is_signal_option(token)
        || is_flag_only_short_cluster(token)
    {
        return Ok(EnvOptionStep::Continue(1));
    }
    if is_separate_value_option(token) {
        // The following operand belongs to env rather than the eventual child process
        return (arguments.len() >= 2)
            .then_some(EnvOptionStep::Continue(2))
            .ok_or("env option is missing its required value");
    }
    if is_working_directory_option(token) {
        return Err("env working-directory options are not portable in preset commands");
    }
    if is_split_string_option(token) {
        return Err("env split-string options are ambiguous in preset commands");
    }
    if token.starts_with('-') {
        return Err("env command contains an unsupported option form");
    }
    Ok(EnvOptionStep::Stop)
}

fn is_flag_only_env_option(token: &str) -> bool {
    matches!(
        token,
        "-" | "-i"
            | "-0"
            | "-v"
            | "--ignore-environment"
            | "--null"
            | "--debug"
            | "--list-signal-handling"
    )
}

fn is_separate_value_option(token: &str) -> bool {
    matches!(token, "-u" | "--unset" | "-a" | "--argv0")
}

fn is_attached_value_option(token: &str) -> bool {
    token.starts_with("--unset=")
        || token.starts_with("--argv0=")
        || attached_short_option(token, 'u')
        || attached_short_option(token, 'a')
}

fn is_signal_option(token: &str) -> bool {
    let name = token.split_once('=').map_or(token, |(name, _)| name);
    matches!(
        name,
        "--block-signal" | "--default-signal" | "--ignore-signal"
    )
}

fn is_working_directory_option(token: &str) -> bool {
    token == "-C"
        || token == "--chdir"
        || token.starts_with("--chdir=")
        || attached_short_option(token, 'C')
}

fn is_split_string_option(token: &str) -> bool {
    token == "-S"
        || token == "--split-string"
        || token.starts_with("--split-string=")
        || attached_short_option(token, 'S')
}

pub(super) fn split_env_assignment(token: &str) -> Option<(&str, &str)> {
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

fn env_path_components<'a>(
    name: &str,
    value: &'a str,
) -> Result<Option<Vec<&'a str>>, &'static str> {
    if is_dynamic_loader_variable(name) && contains_dynamic_loader_token(value) {
        return Err("dynamic loader tokens are not portable in preset commands");
    }
    if matches!(name, "BASH_ENV" | "ENV")
        && value
            .chars()
            .any(|character| matches!(character, '$' | '`' | '~'))
    {
        // Shells expand these startup-file values before opening them
        return Err("shell startup environment paths cannot contain expansions");
    }

    let components = match name {
        // glibc accepts ASCII whitespace or colons with no escaping for preload objects
        "LD_PRELOAD" => value
            .split(|character: char| character == ':' || character.is_ascii_whitespace())
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>(),
        // glibc accepts both directory separators and treats empty fields as the child cwd
        "LD_LIBRARY_PATH" => value.split([':', ';']).collect::<Vec<_>>(),
        // These are colon-separated lists on Unix and empty fields resolve from the child cwd
        "PATH" | "LD_AUDIT" | "PYTHONPATH" | "PERL5LIB" | "RUBYLIB" | "NODE_PATH"
        | "GCONV_PATH" => value.split(':').collect::<Vec<_>>(),
        // These consumers interpret the complete value as one path
        "HOME" | "LD_CONFIG_FILE" | "PYTHONHOME" | "BASH_ENV" | "ENV" | "ZDOTDIR" => {
            vec![value]
        }
        _ => return Ok(None),
    };

    if matches!(name, "LD_PRELOAD" | "LD_AUDIT")
        && components
            .iter()
            .any(|component| !component.is_empty() && !component.contains('/'))
    {
        // Bare object names use the system loader search order rather than the config directory
        return Err("loader object names must use an explicit config-relative path");
    }

    Ok(Some(components))
}

fn is_dynamic_loader_variable(name: &str) -> bool {
    matches!(name, "LD_PRELOAD" | "LD_LIBRARY_PATH" | "LD_AUDIT")
}

fn contains_dynamic_loader_token(value: &str) -> bool {
    [
        "$ORIGIN",
        "${ORIGIN}",
        "$LIB",
        "${LIB}",
        "$PLATFORM",
        "${PLATFORM}",
    ]
    .iter()
    .any(|token| value.contains(token))
}

fn resolve_env_path_value(config_dir: &Path, value: &str) -> PathBuf {
    if value.is_empty() {
        // Empty list components mean cwd for loaders and path-search consumers
        return config_dir.to_path_buf();
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return path;
    }
    config_dir.join(path)
}

pub fn looks_like_path_token(token: &str) -> bool {
    // Every supported relative prefix already contains a path separator
    token == "~" || token.contains('/')
}

pub fn is_host_specific_path_token(token: &str) -> bool {
    token.starts_with('/') || token == "~" || token.starts_with("~/")
}
