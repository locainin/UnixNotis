use std::ffi::{OsStr, OsString};
use std::ops::Range;
use std::path::{Path, PathBuf};

use unixnotis_core::CommandSpec;

use super::super::pathing::{format_relative_path, normalize_lexical_path};

pub fn resolve_command_path_token(config_dir: &Path, command: &CommandSpec) -> Option<PathBuf> {
    let first = effective_program(command)?;
    let first = first.to_str()?;
    if !looks_like_path_token(first) {
        return None;
    }
    let path = PathBuf::from(first);
    if path.is_absolute() {
        return Some(path);
    }
    Some(config_dir.join(path))
}

pub fn collect_outside_env_path_tokens(
    config_dir: &Path,
    command: &CommandSpec,
) -> Vec<(String, PathBuf)> {
    let normalized_root = normalize_lexical_path(config_dir);
    command_env_assignments(command)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(name, value)| {
            let components = env_path_components(&name, &value).ok().flatten()?;
            let outside_path = components
                .into_iter()
                .map(|part| resolve_env_path_value(config_dir, part))
                .find(|path| !normalize_lexical_path(path).starts_with(&normalized_root))?;
            Some((name, outside_path))
        })
        .collect()
}

pub fn rewrite_command_to_config_relative(config_dir: &Path, command: &mut CommandSpec) -> bool {
    let Some(first) = effective_program(command).and_then(OsStr::to_str) else {
        return false;
    };
    if !is_host_specific_path_token(first) {
        return false;
    }
    let Some(resolved_path) = resolve_command_path_token(config_dir, command) else {
        return false;
    };
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_path = normalize_lexical_path(&resolved_path);
    let Ok(relative_path) = normalized_path.strip_prefix(&normalized_root) else {
        return false;
    };
    let rewritten = format_relative_path(relative_path);
    if rewritten.is_empty() {
        return false;
    }

    let CommandSpec::Direct { program, args, .. } = command else {
        return false;
    };
    if program == Path::new("env") {
        let Ok(layout) = env_command_layout(args) else {
            return false;
        };
        let Some(index) = layout.program_index else {
            return false;
        };
        args[index] = OsString::from(rewritten);
    } else {
        *program = PathBuf::from(rewritten);
    }
    true
}

pub fn first_command_token(command: &CommandSpec) -> Option<String> {
    effective_program(command)?.to_str().map(str::to_string)
}

fn command_env_assignments(command: &CommandSpec) -> Result<Vec<(String, String)>, &'static str> {
    let CommandSpec::Direct { program, args, env } = command else {
        return Ok(Vec::new());
    };
    let mut assignments = env
        .iter()
        .map(|(name, value)| {
            Ok((
                name.to_str()
                    .ok_or("environment name is not UTF-8")?
                    .to_string(),
                value
                    .to_str()
                    .ok_or("environment value is not UTF-8")?
                    .to_string(),
            ))
        })
        .collect::<Result<Vec<_>, &'static str>>()?;

    if program == Path::new("env") {
        let layout = env_command_layout(args)?;
        assignments.extend(
            args[layout.assignment_range]
                .iter()
                .map(|token| token.to_str().ok_or("env argument is not UTF-8"))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter_map(split_env_assignment)
                .map(|(name, value)| (name.to_string(), value.to_string())),
        );
    }
    Ok(assignments)
}

fn effective_program(command: &CommandSpec) -> Option<&OsStr> {
    let CommandSpec::Direct { program, args, .. } = command else {
        return None;
    };
    if program != Path::new("env") {
        return Some(program.as_os_str());
    }
    let index = env_command_layout(args).ok()?.program_index?;
    Some(args[index].as_os_str())
}

pub(super) fn validate_env_command_layout(command: &CommandSpec) -> Result<(), &'static str> {
    let CommandSpec::Direct { program, args, .. } = command else {
        return Ok(());
    };
    if program != Path::new("env") {
        return Ok(());
    }
    env_command_layout(args).map(|_| ())
}

pub(super) fn validate_env_path_semantics(command: &CommandSpec) -> Result<(), &'static str> {
    for (name, value) in command_env_assignments(command)? {
        let _ = env_path_components(&name, &value)?;
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

fn env_command_layout(args: &[OsString]) -> Result<EnvCommandLayout, &'static str> {
    let mut option_count = 0usize;
    let mut remaining = args;
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

    let assignment_count = remaining
        .iter()
        .map(|token| token.to_str().ok_or("env argument is not UTF-8"))
        .map_while(Result::ok)
        .take_while(|token| split_env_assignment(token).is_some())
        .count();
    let program_index = option_count
        .checked_add(assignment_count)
        .ok_or("env argument count overflowed")?;
    Ok(EnvCommandLayout {
        assignment_range: option_count..program_index,
        program_index: (program_index < args.len()).then_some(program_index),
    })
}

fn env_option_step(arguments: &[OsString]) -> Result<EnvOptionStep, &'static str> {
    let Some(token) = arguments.first() else {
        return Ok(EnvOptionStep::Stop);
    };
    let token = token.to_str().ok_or("env argument is not UTF-8")?;
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
        return Err("shell startup environment paths cannot contain expansions");
    }

    let components = match name {
        "LD_PRELOAD" => value
            .split(|character: char| character == ':' || character.is_ascii_whitespace())
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>(),
        "LD_LIBRARY_PATH" => value.split([':', ';']).collect::<Vec<_>>(),
        "PATH" | "LD_AUDIT" | "PYTHONPATH" | "PERL5LIB" | "RUBYLIB" | "NODE_PATH"
        | "GCONV_PATH" => value.split(':').collect::<Vec<_>>(),
        "PYTHONHOME" => python_home_components(value)?,
        "HOME" | "LD_CONFIG_FILE" | "BASH_ENV" | "ENV" | "ZDOTDIR" => vec![value],
        _ => return Ok(None),
    };

    if matches!(name, "LD_PRELOAD" | "LD_AUDIT")
        && components
            .iter()
            .any(|component| !component.is_empty() && !component.contains('/'))
    {
        return Err("loader object names must use an explicit config-relative path");
    }
    Ok(Some(components))
}

fn python_home_components(value: &str) -> Result<Vec<&str>, &'static str> {
    let mut parts = value.splitn(3, ':');
    let prefix = parts.next().unwrap_or_default();
    let exec_prefix = parts.next();
    if parts.next().is_some() {
        return Err("PYTHONHOME contains more than one prefix separator");
    }
    match exec_prefix {
        Some(exec_prefix) if !prefix.is_empty() && !exec_prefix.is_empty() => {
            Ok(vec![prefix, exec_prefix])
        }
        Some(_) => Err("PYTHONHOME contains an empty prefix"),
        None => Ok(vec![prefix]),
    }
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
        return config_dir.to_path_buf();
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return path;
    }
    config_dir.join(path)
}

pub fn looks_like_path_token(token: &str) -> bool {
    token.contains('/')
}

pub fn is_host_specific_path_token(token: &str) -> bool {
    token.starts_with('/')
}
