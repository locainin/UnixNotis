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
        assignments.extend(parsed.args.iter().map_while(|token| token.split_once('=')));
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
    parsed
        .args
        .iter()
        .position(|token| !token.contains('=') && !token.starts_with('-'))
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
