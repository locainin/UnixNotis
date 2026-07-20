use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::super::{parse_legacy_command, CommandSpec, LegacyCommandError};

#[test]
fn quoted_shell_punctuation_migrates_to_literal_direct_arguments() {
    let parsed = parse_legacy_command("printf '%s\\n' 'battery|charging'")
        .expect("parse quoted literal command");

    assert_eq!(parsed.program(), Some(Path::new("printf")));
    assert_eq!(
        parsed.args(),
        Some([OsString::from("%s\\n"), OsString::from("battery|charging")].as_slice())
    );
    assert!(!parsed.is_shell());
}

#[test]
fn leading_environment_assignments_migrate_to_direct_environment() {
    let parsed = parse_legacy_command("LANG=C MODE='two words' /bin/printf ok")
        .expect("parse environment command");

    assert_eq!(parsed.program(), Some(Path::new("/bin/printf")));
    let env = parsed.env().expect("direct environment");
    assert_eq!(env.get(OsStr::new("LANG")), Some(&"C".into()));
    assert_eq!(env.get(OsStr::new("MODE")), Some(&"two words".into()));
}

#[test]
fn real_shell_operators_remain_explicit_shell_scripts() {
    for command in [
        "producer | parser",
        "first && second",
        "echo $HOME",
        "printf '%s' \"$HOME\"",
        "echo *.png",
    ] {
        assert_eq!(
            parse_legacy_command(command).expect("parse shell command"),
            CommandSpec::shell(command),
            "{command}"
        );
    }
}

#[test]
fn legacy_shell_c_wrapper_migrates_to_the_inner_explicit_script() {
    assert_eq!(
        parse_legacy_command("sh -c 'producer | parser'").expect("parse shell wrapper"),
        CommandSpec::shell("producer | parser")
    );
}

#[test]
fn shell_wrappers_with_environment_or_extra_arguments_remain_direct() {
    for command in ["MODE=safe sh -c 'exit 0'", "sh -c 'exit 0' extra"] {
        let parsed = parse_legacy_command(command).expect("parse shell wrapper");

        assert!(!parsed.is_shell(), "{command}");
        assert!(parsed.invokes_shell(), "{command}");
    }
}

#[test]
fn ordinary_two_argument_commands_never_become_shell_scripts() {
    let parsed = parse_legacy_command("printf -c literal").expect("parse direct command");

    assert!(!parsed.is_shell());
    assert_eq!(parsed.program(), Some(Path::new("printf")));
}

#[test]
fn escaped_metacharacters_and_runtime_placeholders_stay_direct() {
    for command in [
        r"printf battery\|charging",
        "wpctl set-volume sink {value}%",
        r"printf \$HOME",
    ] {
        assert!(
            !parse_legacy_command(command)
                .expect("parse direct command")
                .is_shell(),
            "{command}"
        );
    }
}

#[test]
fn escaped_quotes_do_not_expose_literal_shell_punctuation() {
    for command in [
        r#"printf "literal\"|value""#,
        r"printf 'literal|value'",
        r"printf literal\|value",
    ] {
        assert!(
            !parse_legacy_command(command)
                .expect("parse quoted direct command")
                .is_shell(),
            "{command}"
        );
    }
}

#[test]
fn shell_expansion_inside_double_quotes_remains_shell_mode() {
    for command in [r#"printf "$HOME""#, r#"printf "`pwd`""#] {
        assert!(
            parse_legacy_command(command)
                .expect("parse expanded command")
                .is_shell(),
            "{command}"
        );
    }
}

#[test]
fn comments_and_history_expansion_only_trigger_at_token_boundaries() {
    for command in ["printf value#suffix", "printf value!suffix"] {
        assert!(
            !parse_legacy_command(command)
                .expect("parse literal token")
                .is_shell(),
            "{command}"
        );
    }
    for command in ["printf value # comment", "printf value ! history"] {
        assert!(
            parse_legacy_command(command)
                .expect("parse shell token")
                .is_shell(),
            "{command}"
        );
    }
}

#[test]
fn unknown_or_unbalanced_braces_require_explicit_shell_mode() {
    for command in ["printf {other}", "printf value}", "printf {value}{other}"] {
        assert!(
            parse_legacy_command(command)
                .expect("parse brace command")
                .is_shell(),
            "{command}"
        );
    }

    assert!(!parse_legacy_command("printf {value}")
        .expect("parse runtime placeholder")
        .is_shell());
}

#[test]
fn environment_assignment_names_follow_portable_identifier_rules() {
    let parsed = parse_legacy_command("_A=1 A2=two /bin/true").expect("parse valid assignments");
    let env = parsed.env().expect("direct environment");
    assert_eq!(env.get(OsStr::new("_A")), Some(&OsString::from("1")));
    assert_eq!(env.get(OsStr::new("A2")), Some(&OsString::from("two")));

    for command in [
        "1A=value /bin/true",
        "A-B=value /bin/true",
        "=value /bin/true",
    ] {
        let parsed = parse_legacy_command(command).expect("parse non-assignment token");
        assert_eq!(
            parsed.program(),
            Some(Path::new(command.split_whitespace().next().unwrap()))
        );
        assert!(parsed.env().expect("direct environment").is_empty());
    }
}

#[test]
fn invalid_legacy_commands_fail_closed() {
    assert_eq!(parse_legacy_command("   "), Err(LegacyCommandError::Empty));
    assert!(matches!(
        parse_legacy_command("echo 'unterminated"),
        Err(LegacyCommandError::Malformed(_))
    ));
    assert_eq!(
        parse_legacy_command("NAME=value"),
        Err(LegacyCommandError::MissingProgram)
    );
}
