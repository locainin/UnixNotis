use super::launch::trial_launch_script;
use super::paths::shell_quote;

#[test]
fn trial_launch_script_guards_cleanup_with_expected_symlink_target() {
    let script = trial_launch_script(
        "'/tmp/unixnotis-daemon'",
        "'/home/user/.local/bin/noticenterctl'",
        "'/tmp/target/debug/noticenterctl'",
    );

    // Signal-time cleanup must not be a blind rm of whatever is at the shim path
    assert!(script.contains("[ -L '/home/user/.local/bin/noticenterctl' ]"));
    assert!(script.contains("readlink -- '/home/user/.local/bin/noticenterctl'"));
    assert!(script.contains("= '/tmp/target/debug/noticenterctl'"));
    assert!(script.contains("rm -f -- '/home/user/.local/bin/noticenterctl'"));
}

#[test]
fn shell_quote_preserves_spaces_and_embedded_single_quotes() {
    let quoted = shell_quote("/tmp/unix notis/it's fine");

    // POSIX single-quote escaping closes, emits a quoted single quote, then reopens
    assert_eq!(quoted, "'/tmp/unix notis/it'\"'\"'s fine'");
}
