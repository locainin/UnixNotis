use super::*;

#[test]
fn sanitize_log_value_strips_newlines_and_caps() {
    let value = "ab\ncd\rEF";
    let sanitized = sanitize_log_value(value, 5);
    assert_eq!(sanitized, "ab cd...");

    let no_truncate = sanitize_log_value("ok", 5);
    assert_eq!(no_truncate, "ok");
}

#[test]
fn sanitize_log_value_strips_bidi_controls() {
    let value = "safe\u{202E}spoof\u{2066}text\u{2069}";
    let sanitized = sanitize_log_value(value, 80);
    assert_eq!(sanitized, "safespooftext");
}

#[test]
fn sanitize_display_text_strips_bidi_controls_and_preserves_newlines() {
    // Newlines stay, bidi marks do not
    let value = "safe\u{202E}name\nnext\u{2066}line\u{2069}";
    let sanitized = sanitize_display_text(value);
    assert_eq!(sanitized, "safename\nnextline");
}

#[test]
fn sanitize_inline_display_text_flattens_control_characters() {
    // Inline text stays on one row
    let value = "fake\tname\nrow\u{202E}";
    let sanitized = sanitize_inline_display_text(value);
    assert_eq!(sanitized, "fake name row");
}

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
fn diagnostic_mode_parses_expected_values() {
    assert!(diagnostic_mode_from(Some("1")));
    assert!(diagnostic_mode_from(Some("true")));
    assert!(diagnostic_mode_from(Some("YES")));
    assert!(diagnostic_mode_from(Some("on")));
    assert!(!diagnostic_mode_from(Some("0")));
    assert!(!diagnostic_mode_from(Some("false")));
    assert!(!diagnostic_mode_from(None));
}

#[test]
fn log_limit_respects_mode() {
    assert_eq!(log_limit_for(false), DEFAULT_LOG_LIMIT);
    assert_eq!(log_limit_for(true), DIAGNOSTIC_LOG_LIMIT);
}
