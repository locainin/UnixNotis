use super::*;
use std::fs;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn unique_temp_path(name: &str) -> PathBuf {
    // Use the process id and a monotonic-ish timestamp so parallel test binaries do not collide
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "unixnotis-core-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // Environment variables are process-global, so tests that mutate them must serialize
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock should not be poisoned")
}

fn set_env(key: &str, value: Option<&str>) -> Option<String> {
    let previous = env::var(key).ok();
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

fn restore_env(key: &str, previous: Option<String>) {
    match previous {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}

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

#[test]
fn executable_path_accepts_only_regular_executable_files() {
    let path = unique_temp_path("executable-file");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("test executable should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("test executable permissions should be writable");

    assert!(is_executable_path(&path));

    let _ = fs::remove_file(path);
}

#[test]
fn executable_path_rejects_non_executable_regular_files() {
    let path = unique_temp_path("plain-file");
    fs::write(&path, "not executable").expect("test file should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("test file permissions should be writable");

    #[cfg(unix)]
    assert!(!is_executable_path(&path));

    let _ = fs::remove_file(path);
}

#[test]
fn executable_path_rejects_directories_and_missing_paths() {
    let dir = unique_temp_path("dir");
    fs::create_dir(&dir).expect("test directory should be writable");
    let missing = dir.join("missing");

    assert!(!is_executable_path(&dir));
    assert!(!is_executable_path(&missing));

    let _ = fs::remove_dir(dir);
}

#[test]
fn program_lookup_with_explicit_path_uses_executable_rules() {
    let path = unique_temp_path("explicit-program");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("test program should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("test program permissions should be writable");

    assert!(program_in_path(path.to_string_lossy().as_ref()));

    let _ = fs::remove_file(path);
}

#[test]
fn program_lookup_with_explicit_path_rejects_non_executable_file() {
    let path = unique_temp_path("explicit-non-executable");
    fs::write(&path, "plain").expect("test program should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("test program permissions should be writable");

    #[cfg(unix)]
    assert!(!program_in_path(path.to_string_lossy().as_ref()));

    let _ = fs::remove_file(path);
}

#[test]
fn program_lookup_cache_refreshes_when_path_changes() {
    let _guard = env_lock();
    let root = unique_temp_path("path-cache");
    let bin_a = root.join("a");
    let bin_b = root.join("b");
    fs::create_dir_all(&bin_a).expect("first bin dir");
    fs::create_dir_all(&bin_b).expect("second bin dir");
    let program = format!("unixnotis-test-tool-{}", std::process::id());
    let first = bin_a.join(&program);
    let second = bin_b.join(&program);
    fs::write(&first, "#!/bin/sh\nexit 0\n").expect("first program");
    fs::write(&second, "#!/bin/sh\nexit 0\n").expect("second program");

    #[cfg(unix)]
    {
        fs::set_permissions(&first, fs::Permissions::from_mode(0o755))
            .expect("first program permissions");
        fs::set_permissions(&second, fs::Permissions::from_mode(0o755))
            .expect("second program permissions");
    }

    let previous = set_env("PATH", Some(bin_a.to_string_lossy().as_ref()));
    assert!(program_in_path(&program));

    // Removing the first file would leave a stale true result if PATH changes were ignored
    fs::remove_file(&first).expect("remove first program");
    env::set_var("PATH", bin_b.to_string_lossy().as_ref());
    assert!(program_in_path(&program));

    // A new PATH with no executable must also clear a cached true result
    env::set_var("PATH", root.to_string_lossy().as_ref());
    assert!(!program_in_path(&program));

    restore_env("PATH", previous);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_state_dir_reads_environment_wrapper() {
    let _guard = env_lock();
    let root = unique_temp_path("state-env");
    let state = root.join("state");
    let home = root.join("home");
    let old_state = set_env("XDG_STATE_HOME", Some(state.to_string_lossy().as_ref()));
    let old_home = set_env("HOME", Some(home.to_string_lossy().as_ref()));

    assert_eq!(resolve_state_dir(), Some(state));

    restore_env("XDG_STATE_HOME", old_state);
    restore_env("HOME", old_home);
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
fn simple_command_accepts_plain_program_and_arguments() {
    assert!(is_simple_command("notify-send hello world"));
    assert!(is_simple_command("/usr/bin/notify-send hello"));
    assert!(is_simple_command("./local-helper --flag value"));
}

#[test]
fn simple_command_rejects_shell_meta_characters_and_newlines() {
    for command in [
        "echo hi | wc -l",
        "echo hi && echo bye",
        "echo hi; rm -rf x",
        "echo $(date)",
        "echo `date`",
        "echo ~/file",
        "echo one\necho two",
        "echo one\recho two",
    ] {
        assert!(
            !is_simple_command(command),
            "command should need a shell: {command}"
        );
    }
}

#[test]
fn simple_command_rejects_leading_env_assignment_without_explicit_path() {
    assert!(!is_simple_command("FOO=bar notify-send hi"));
    assert!(is_simple_command("/tmp/FOO=bar notify-send hi"));
    assert!(is_simple_command("./FOO=bar notify-send hi"));
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
fn diagnostic_mode_reads_environment_wrapper() {
    let _guard = env_lock();
    let previous = set_env("UNIXNOTIS_DIAGNOSTIC", Some("yes"));
    assert!(diagnostic_mode());

    env::set_var("UNIXNOTIS_DIAGNOSTIC", "off");
    assert!(!diagnostic_mode());

    restore_env("UNIXNOTIS_DIAGNOSTIC", previous);
}

#[test]
fn log_limit_respects_mode() {
    assert_eq!(log_limit_for(false), DEFAULT_LOG_LIMIT);
    assert_eq!(log_limit_for(true), DIAGNOSTIC_LOG_LIMIT);
}

#[test]
fn log_limit_and_snippet_use_diagnostic_environment() {
    let _guard = env_lock();
    let previous = set_env("UNIXNOTIS_DIAGNOSTIC", Some("true"));
    assert_eq!(log_limit(), DIAGNOSTIC_LOG_LIMIT);

    env::set_var("UNIXNOTIS_DIAGNOSTIC", "false");
    assert_eq!(log_limit(), DEFAULT_LOG_LIMIT);

    let noisy = "value\nwith\rcontrols";
    assert_eq!(log_snippet(noisy), "value with controls");

    restore_env("UNIXNOTIS_DIAGNOSTIC", previous);
}

#[test]
fn sanitize_log_value_replaces_newlines_and_other_controls() {
    let value = "a\nb\rc\u{0007}d";
    assert_eq!(sanitize_log_value(value, 80), "a b c d");
}

#[test]
fn sanitize_display_text_maps_tabs_and_carriage_returns_separately() {
    let value = "a\tb\rc\nnext";
    assert_eq!(sanitize_display_text(value), "a b c\nnext");
    assert_eq!(sanitize_inline_display_text(value), "a b c next");
}

#[test]
fn diagnostic_limits_are_distinct_and_ordered() {
    // Diagnostic mode intentionally keeps longer snippets for manual troubleshooting
    assert_eq!(default_log_limit(), DEFAULT_LOG_LIMIT);
    assert_eq!(diagnostic_log_limit(), DIAGNOSTIC_LOG_LIMIT);
    assert!(diagnostic_log_limit() > default_log_limit());
}
