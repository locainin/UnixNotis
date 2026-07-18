//! XDG and home-relative path resolution helpers

use std::borrow::Cow;
use std::env;
use std::path::PathBuf;

pub const CONFIG_PATH_ENV: &str = "UNIXNOTIS_CONFIG_PATH";

/// Resolve `XDG_STATE_HOME` with the specification defaults
#[must_use]
pub fn resolve_state_dir() -> Option<PathBuf> {
    resolve_state_dir_from_env(
        env::var("XDG_STATE_HOME").ok().as_deref(),
        env::var("HOME").ok().as_deref(),
    )
}

/// Resolve the state directory from explicit environment values
#[must_use]
pub fn resolve_state_dir_from_env(
    xdg_state_home: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    if let Some(dir) = xdg_state_home {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.is_absolute() {
                return Some(path);
            }
        }
    }
    let home = home?;
    let trimmed = home.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return None;
    }
    Some(path.join(".local").join("state"))
}

/// Expand leading `~`/`~/` to $HOME, preserving other paths as-is
#[must_use]
pub fn expand_tilde(value: &str) -> Cow<'_, str> {
    let trimmed = value.trim();
    if trimmed == "~" || trimmed.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            if trimmed == "~" {
                return home.into();
            }
            let suffix = trimmed.trim_start_matches("~/");
            return format!("{home}/{suffix}").into();
        }
    }
    value.into()
}

#[cfg(test)]
#[path = "tests/paths.rs"]
mod tests;
