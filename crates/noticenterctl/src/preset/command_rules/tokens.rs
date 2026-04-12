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

    let first = first_command_token(trimmed)?;
    if !looks_like_path_token(first) {
        return None;
    }

    let expanded = PathBuf::from(util::expand_tilde(first).into_owned());
    if expanded.is_absolute() {
        return Some(expanded);
    }
    Some(config_dir.join(expanded))
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
