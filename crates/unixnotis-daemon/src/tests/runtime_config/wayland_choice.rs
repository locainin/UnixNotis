use crate::test_support::{env_lock, EnvVarGuard};

use super::{apply_wayland_env, choose_wayland_fallback};

#[test]
fn choose_wayland_fallback_prefers_wayland_zero() {
    let chosen = choose_wayland_fallback(vec![
        "wayland-2".to_string(),
        "wayland-0".to_string(),
        "wayland-1".to_string(),
    ]);

    // The conventional primary socket should win when it exists
    assert_eq!(chosen.as_deref(), Some("wayland-0"));
}

#[test]
fn choose_wayland_fallback_rejects_multiple_nonzero_candidates() {
    let chosen = choose_wayland_fallback(vec![
        "wayland-7".to_string(),
        "wayland-3".to_string(),
        "wayland-5".to_string(),
    ]);

    // Ambiguous sockets can point at the wrong display, so force explicit env
    assert_eq!(chosen.as_deref(), None);
}

#[test]
fn choose_wayland_fallback_accepts_single_nonzero_candidate() {
    let chosen = choose_wayland_fallback(vec!["wayland-3".to_string()]);

    // A single discovered socket is unambiguous when wayland-0 is absent
    assert_eq!(chosen.as_deref(), Some("wayland-3"));
}

#[test]
fn choose_wayland_fallback_dedupes_before_checking_ambiguity() {
    let chosen = choose_wayland_fallback(vec!["wayland-3".to_string(), "wayland-3".to_string()]);

    // Duplicate directory entries should not make one display look ambiguous
    assert_eq!(chosen.as_deref(), Some("wayland-3"));
}

#[test]
fn choose_wayland_fallback_returns_none_for_empty_list() {
    assert!(choose_wayland_fallback(Vec::new()).is_none());
}

#[test]
fn apply_wayland_env_sets_display_and_default_session_type() {
    let _guard = env_lock();
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvVarGuard::remove("XDG_SESSION_TYPE");

    apply_wayland_env("wayland-test");

    assert_eq!(
        std::env::var("WAYLAND_DISPLAY").as_deref(),
        Ok("wayland-test")
    );
    assert_eq!(std::env::var("XDG_SESSION_TYPE").as_deref(), Ok("wayland"));
}

#[test]
fn apply_wayland_env_preserves_existing_session_type() {
    let _guard = env_lock();
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvVarGuard::set("XDG_SESSION_TYPE", "wayland-custom");

    apply_wayland_env("wayland-test");

    assert_eq!(
        std::env::var("WAYLAND_DISPLAY").as_deref(),
        Ok("wayland-test")
    );
    assert_eq!(
        std::env::var("XDG_SESSION_TYPE").as_deref(),
        Ok("wayland-custom")
    );
}
