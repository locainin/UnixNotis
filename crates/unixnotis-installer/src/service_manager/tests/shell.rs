use std::path::Path;

use super::shell::{
    envdir_file_contents, envdir_sync_prelude, is_safe_env_name, render_envdir_shell_update,
    shell_quote,
};

#[test]
fn envdir_sync_prelude_renders_readable_guard_steps() {
    let steps = envdir_sync_prelude(Path::new("/tmp/service root/env"));

    assert_eq!(
        steps,
        [
            "umask 077",
            "envdir='/tmp/service root/env'",
            r#"[ ! -L "$envdir" ] || exit 1"#,
            r#"mkdir -p "$envdir" || exit 1"#,
            r#"[ -d "$envdir" ] && [ ! -L "$envdir" ] || exit 1"#,
        ]
    );
}

#[test]
fn envdir_shell_update_writes_temp_file_before_replacing_target() {
    let update = render_envdir_shell_update("WAYLAND_DISPLAY");

    assert!(update.contains(r#"mktemp "$envdir/.WAYLAND_DISPLAY.XXXXXX""#));
    assert!(update.contains(r#"printenv WAYLAND_DISPLAY > "$tmp" || : > "$tmp""#));
    assert!(update.contains(r#"chmod 600 "$tmp""#));
    assert!(update.contains(r#"mv -f "$tmp" "$envdir/WAYLAND_DISPLAY""#));
}

#[test]
fn envdir_file_contents_match_envdir_first_line_semantics() {
    assert_eq!(
        envdir_file_contents(Some("wayland-1\nignored")),
        "wayland-1\n"
    );
    assert_eq!(
        envdir_file_contents(Some("/run/user/1000\t ")),
        "/run/user/1000\n"
    );
    assert_eq!(envdir_file_contents(None), "");
}

#[test]
fn safe_env_name_accepts_shell_variable_names_only() {
    assert!(is_safe_env_name("WAYLAND_DISPLAY"));
    assert!(is_safe_env_name("_UNIXNOTIS_TEST"));
    assert!(!is_safe_env_name(""));
    assert!(!is_safe_env_name("1DISPLAY"));
    assert!(!is_safe_env_name("WAYLAND-DISPLAY"));
    assert!(!is_safe_env_name("WAYLAND/DISPLAY"));
}

#[test]
fn shell_quote_escapes_single_quotes() {
    assert_eq!(shell_quote(""), "''");
    assert_eq!(shell_quote("plain"), "'plain'");
    assert_eq!(shell_quote("quote'path"), "'quote'\\''path'");
}
