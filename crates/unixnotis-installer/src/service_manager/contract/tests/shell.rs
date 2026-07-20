use super::super::shell::{envdir_file_contents, is_safe_env_name, shell_quote};

#[test]
fn envdir_file_contents_match_envdir_first_line_semantics() {
    // chpst and s6-envdir ignore everything after the first newline
    assert_eq!(
        envdir_file_contents(Some("wayland-1\nignored")),
        "wayland-1\n"
    );
    // Trailing blanks are stripped so env files do not preserve accidental shell padding
    assert_eq!(
        envdir_file_contents(Some("/run/user/1000\t ")),
        "/run/user/1000\n"
    );
    assert_eq!(envdir_file_contents(None), "");
}

#[test]
fn safe_env_name_accepts_shell_variable_names_only() {
    // Restrict names to shell variable syntax because names are interpolated into shell fragments
    assert!(is_safe_env_name("WAYLAND_DISPLAY"));
    assert!(is_safe_env_name("_UNIXNOTIS_TEST"));
    assert!(!is_safe_env_name(""));
    assert!(!is_safe_env_name("1DISPLAY"));
    assert!(!is_safe_env_name("WAYLAND-DISPLAY"));
    assert!(!is_safe_env_name("WAYLAND/DISPLAY"));
}

#[test]
fn shell_quote_escapes_single_quotes() {
    // POSIX shell quoting uses the close-escape-open sequence for embedded single quotes
    assert_eq!(shell_quote(""), "''");
    assert_eq!(shell_quote("plain"), "'plain'");
    assert_eq!(shell_quote("quote'path"), "'quote'\\''path'");
}
