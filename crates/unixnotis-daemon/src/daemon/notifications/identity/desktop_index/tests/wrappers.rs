use super::{normalize_launch_command, ExecParseError};
use crate::daemon::notifications::identity::desktop_index::model::LaunchWrapper;

#[test]
fn env_wrapper_preserves_environment_and_exposes_the_application_command() {
    let normalized = normalize_launch_command(
        [
            "/usr/bin/env",
            "-i",
            "-u",
            "OLD_VALUE",
            "FEATURE=1",
            "--",
            "example-app",
            "--fixed",
            "%u",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
    )
    .expect("normalize env command");

    assert_eq!(normalized.executable, "example-app");
    assert_eq!(normalized.arguments, ["--fixed", "%u"]);
    assert_eq!(
        normalized.environment,
        vec![(b"FEATURE".to_vec(), b"1".to_vec())]
    );
    assert_eq!(normalized.wrappers, [LaunchWrapper::Env]);
}

#[test]
fn nested_env_wrappers_are_normalized_without_application_specific_rules() {
    let normalized = normalize_launch_command(
        ["env", "A=1", "/usr/bin/env", "B=2", "example-app"]
            .into_iter()
            .map(str::to_string)
            .collect(),
    )
    .expect("normalize nested env command");

    assert_eq!(normalized.executable, "example-app");
    assert_eq!(normalized.environment.len(), 2);
    assert_eq!(
        normalized.wrappers,
        [LaunchWrapper::Env, LaunchWrapper::Env]
    );
}

#[test]
fn unsupported_or_incomplete_env_syntax_fails_closed() {
    for (tokens, expected) in [
        (
            vec!["env".to_string(), "-S".to_string(), "app".to_string()],
            ExecParseError::UnsupportedWrapper,
        ),
        (
            vec!["env".to_string(), "-u".to_string()],
            ExecParseError::MalformedEnvCommand,
        ),
        (
            vec!["env".to_string(), "FEATURE=1".to_string()],
            ExecParseError::MissingWrappedCommand,
        ),
    ] {
        assert_eq!(normalize_launch_command(tokens), Err(expected));
    }
}
