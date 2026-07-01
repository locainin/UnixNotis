use super::*;

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
