use super::is_probably_slow;
use unixnotis_core::CommandSpec;

#[test]
fn slow_classification_uses_the_structured_program() {
    assert!(is_probably_slow(&CommandSpec::direct("sleep", ["1"])));
    assert!(is_probably_slow(&CommandSpec::direct(
        "nmcli",
        ["radio", "wifi"]
    )));
    assert!(!is_probably_slow(
        &CommandSpec::direct("echo", ["ok"]).with_env("FOO", "bar")
    ));
    assert!(!is_probably_slow(&CommandSpec::direct(
        "echo",
        ["I am not sleeping"]
    )));
}

#[test]
fn explicit_shell_commands_use_the_slow_lane() {
    assert!(is_probably_slow(&CommandSpec::shell("printf ready")));
}

#[test]
fn direct_shell_wrappers_share_the_slow_lane_classification() {
    for shell in ["sh", "ash", "bash", "dash", "fish", "ksh", "zsh"] {
        assert!(
            is_probably_slow(&CommandSpec::direct(shell, ["-c", "sleep 1"])),
            "{shell} -c must receive the slow command budget"
        );
    }

    assert!(is_probably_slow(&CommandSpec::direct(
        "/bin/dash",
        ["-c", "printf ready"]
    )));
    assert!(!is_probably_slow(&CommandSpec::direct(
        "dash",
        ["-x", "script"]
    )));
}
