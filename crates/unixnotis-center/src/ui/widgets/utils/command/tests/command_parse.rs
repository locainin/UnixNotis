use super::{is_probably_slow, parse_simple_command};

#[test]
fn parse_simple_command_honors_quotes() {
    let parsed = parse_simple_command("notify-send \"Hello World\"").expect("parsed command");
    assert_eq!(parsed.program, "notify-send");
    assert!(parsed.env.is_empty());
    assert_eq!(parsed.args, vec!["Hello World"]);
}

#[test]
fn parse_simple_command_rejects_shell_meta() {
    assert!(parse_simple_command("echo hi | wc -l").is_none());
}

#[test]
fn parse_simple_command_accepts_leading_env_assignments() {
    let parsed = parse_simple_command("FOO=bar BAR='two words' notify-send done").expect("parsed");

    assert_eq!(parsed.program, "notify-send");
    assert_eq!(
        parsed.env,
        vec![
            ("FOO".to_string(), "bar".to_string()),
            ("BAR".to_string(), "two words".to_string())
        ]
    );
    assert_eq!(parsed.args, vec!["done"]);
}

#[test]
fn is_probably_slow_respects_program_tokens() {
    assert!(is_probably_slow("sleep 1"));
    assert!(is_probably_slow("nmcli radio wifi"));
    assert!(!is_probably_slow("FOO=bar echo ok"));
    assert!(!is_probably_slow("echo \"I am not sleeping\""));
}

#[test]
fn is_probably_slow_handles_shell_sleep_script() {
    assert!(is_probably_slow("bash -c \"sleep 1\""));
    assert!(!is_probably_slow("bash -c \"echo sleep\""));
}
