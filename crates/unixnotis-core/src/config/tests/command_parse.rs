use super::{parse_command, CommandParseError, ExecutionMode};

#[test]
fn quoted_assignments_and_arguments_are_unquoted_once() {
    let parsed = parse_command("LD_PRELOAD=\"/tmp/evil.so\" VAR='two words' /bin/true done")
        .expect("parse quoted command");

    assert_eq!(
        parsed.env,
        vec![
            ("LD_PRELOAD".to_string(), "/tmp/evil.so".to_string()),
            ("VAR".to_string(), "two words".to_string()),
        ]
    );
    assert_eq!(parsed.program, "/bin/true");
    assert_eq!(parsed.args, vec!["done"]);
    assert_eq!(parsed.execution_mode, ExecutionMode::Direct);
}

#[test]
fn tilde_and_shell_operators_are_classified_for_shell_execution() {
    assert_eq!(
        parse_command("~/bin/probe")
            .expect("parse tilde command")
            .execution_mode,
        ExecutionMode::Shell
    );
    assert_eq!(
        parse_command("echo ok | wc -l")
            .expect("parse pipeline")
            .execution_mode,
        ExecutionMode::Shell
    );
}

#[test]
fn malformed_quoting_and_assignment_only_commands_are_rejected() {
    assert!(matches!(
        parse_command("echo \"unterminated"),
        Err(CommandParseError::Malformed(_))
    ));
    assert_eq!(
        parse_command("HOME=/tmp"),
        Err(CommandParseError::MissingProgram)
    );
}

#[test]
fn invalid_environment_names_remain_program_tokens() {
    let parsed = parse_command("1INVALID=value /bin/true").expect("parse invalid assignment name");

    assert!(parsed.env.is_empty());
    assert_eq!(parsed.program, "1INVALID=value");
}

#[test]
fn carriage_returns_force_shell_execution() {
    let parsed = parse_command("echo one\recho two").expect("parse carriage return command");

    assert_eq!(parsed.execution_mode, ExecutionMode::Shell);
}

#[test]
fn escaped_spaces_remain_inside_the_program_token() {
    let parsed = parse_command("scripts/escaped\\ path/tool --check").expect("parse escaped path");

    assert_eq!(parsed.program, "scripts/escaped path/tool");
    assert_eq!(parsed.args, vec!["--check"]);
}
