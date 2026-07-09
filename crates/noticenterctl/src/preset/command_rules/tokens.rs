use std::path::{Path, PathBuf};

use unixnotis_core::util;

use super::super::pathing::{format_relative_path, normalize_lexical_path};

pub(crate) fn resolve_command_path_token(config_dir: &Path, command: &str) -> Option<PathBuf> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Shell-backed commands can hide paths in many places, so this check only targets
    // explicit path commands where the executable itself is a path token
    if !util::is_simple_command(trimmed) {
        return None;
    }

    let first = first_executable_token(trimmed)?;
    if !looks_like_path_token(first) {
        return None;
    }

    let expanded = PathBuf::from(util::expand_tilde(first).into_owned());
    if expanded.is_absolute() {
        return Some(expanded);
    }
    Some(config_dir.join(expanded))
}

pub(crate) fn collect_outside_env_path_tokens(
    config_dir: &Path,
    command: &str,
) -> Vec<(String, PathBuf)> {
    let trimmed = command.trim();
    if trimmed.is_empty() || !is_safe_for_env_scan(trimmed) {
        return Vec::new();
    }

    let normalized_root = normalize_lexical_path(config_dir);
    leading_env_assignments(trimmed)
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

pub(crate) fn rewrite_command_to_config_relative(
    config_dir: &Path,
    command: &str,
) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() || !util::is_simple_command(trimmed) {
        return None;
    }

    let first = first_command_token(trimmed)?;
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

    // Keep the rest of the command string as-is so flags and placeholders survive
    let rest = trimmed[first.len()..].trim_start();
    if rest.is_empty() {
        return Some(rewritten_first);
    }
    Some(format!("{rewritten_first} {rest}"))
}

pub(crate) fn first_command_token(command: &str) -> Option<&str> {
    command.split_whitespace().next()
}

fn first_executable_token(command: &str) -> Option<&str> {
    command
        .split_whitespace()
        .find(|token| split_env_assignment(token).is_none())
}

fn leading_env_assignments(command: &str) -> impl Iterator<Item = (&str, &str)> {
    command
        .split_whitespace()
        .map_while(|token| split_env_assignment(token))
}

fn split_env_assignment(token: &str) -> Option<(&str, &str)> {
    let (name, value) = token.split_once('=')?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    Some((name, value))
}

fn is_safe_for_env_scan(command: &str) -> bool {
    !command
        .chars()
        .any(|ch| (util::SHELL_META_CHARS.contains(&ch) && ch != '~') || ch == '\n' || ch == '\r')
}

fn env_name_needs_path_validation(name: &str) -> bool {
    matches!(
        name,
        "PATH"
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

pub(crate) fn looks_like_path_token(token: &str) -> bool {
    token == "~"
        || token.starts_with("~/")
        || token.starts_with("./")
        || token.starts_with("../")
        || token.contains('/')
}

pub(crate) fn is_host_specific_path_token(token: &str) -> bool {
    token.starts_with('/') || token == "~" || token.starts_with("~/")
}
