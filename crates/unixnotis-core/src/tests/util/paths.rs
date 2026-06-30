use std::env;
use std::path::PathBuf;

use super::*;

#[test]
fn resolve_state_dir_prefers_xdg_when_absolute() {
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.trim().is_empty() {
        return;
    }
    let xdg = PathBuf::from(&home).join(".state-test");
    let dir = resolve_state_dir_from_env(Some(xdg.to_string_lossy().as_ref()), Some(home.as_str()));
    assert_eq!(dir, Some(xdg));
}

#[test]
fn resolve_state_dir_rejects_relative_home() {
    // A relative HOME cannot form a safe XDG fallback path
    let dir = resolve_state_dir_from_env(None, Some("relative-home"));
    assert_eq!(dir, None);
}

#[test]
fn resolve_state_dir_rejects_blank_home() {
    // Blank values should behave like a missing HOME instead of producing ".local/state"
    let dir = resolve_state_dir_from_env(None, Some("   "));
    assert_eq!(dir, None);
}

#[test]
fn resolve_state_dir_ignores_blank_xdg_and_uses_absolute_home() {
    let home = "/tmp/unixnotis-home";
    let dir = resolve_state_dir_from_env(Some("  "), Some(home));
    assert_eq!(dir, Some(PathBuf::from(home).join(".local").join("state")));
}

#[test]
fn resolve_state_dir_ignores_relative_xdg() {
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.trim().is_empty() {
        return;
    }
    let dir = resolve_state_dir_from_env(Some("state-root"), Some(home.as_str()));
    assert_eq!(dir, Some(PathBuf::from(&home).join(".local").join("state")));
}

#[test]
fn resolve_state_dir_reads_environment_wrapper() {
    let _guard = crate::test_support::test_env_lock();
    let root = crate::test_support::unique_temp_path("state-env");
    let state = root.join("state");
    let home = root.join("home");
    let old_state =
        crate::test_support::set_env("XDG_STATE_HOME", Some(state.to_string_lossy().as_ref()));
    let old_home = crate::test_support::set_env("HOME", Some(home.to_string_lossy().as_ref()));

    assert_eq!(resolve_state_dir(), Some(state));

    crate::test_support::restore_env("XDG_STATE_HOME", old_state);
    crate::test_support::restore_env("HOME", old_home);
}

#[test]
fn resolve_state_dir_falls_back_to_home() {
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.trim().is_empty() {
        return;
    }
    let dir = resolve_state_dir_from_env(Some(""), Some(home.as_str()));
    assert_eq!(dir, Some(PathBuf::from(&home).join(".local").join("state")));
}

#[test]
fn expand_tilde_expands_home_only_for_leading_shell_home_marker() {
    let Ok(home) = env::var("HOME") else {
        return;
    };

    assert_eq!(expand_tilde("~").as_ref(), home);
    assert_eq!(expand_tilde("~/state").as_ref(), format!("{home}/state"));

    // Embedded tildes are literal text, not shell syntax
    assert_eq!(expand_tilde("/tmp/~user").as_ref(), "/tmp/~user");
    assert_eq!(expand_tilde("~other/file").as_ref(), "~other/file");
}
