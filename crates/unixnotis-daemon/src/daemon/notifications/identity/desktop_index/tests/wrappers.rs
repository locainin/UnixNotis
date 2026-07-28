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

#[test]
fn each_supported_env_control_advances_to_the_wrapped_command() {
    for tokens in [
        vec!["env", "-i", "example-app"],
        vec!["env", "--ignore-environment", "example-app"],
        vec!["env", "-u", "OLD_VALUE", "example-app"],
        vec!["env", "--unset=OLD_VALUE", "example-app"],
        vec!["env", "--", "example-app"],
    ] {
        let normalized =
            normalize_launch_command(tokens.into_iter().map(str::to_string).collect::<Vec<_>>())
                .expect("supported env control should expose wrapped command");

        assert_eq!(normalized.executable, "example-app");
        assert!(normalized.arguments.is_empty());
    }
}

#[test]
fn environment_names_follow_portable_identifier_rules() {
    for accepted in ["A=1", "_A=1", "A_1=value", "A="] {
        let normalized = normalize_launch_command(vec![
            "env".to_string(),
            accepted.to_string(),
            "example-app".to_string(),
        ])
        .expect("portable environment assignment");

        assert_eq!(normalized.environment.len(), 1, "{accepted}");
    }

    for rejected in ["1A=value", "=value", "A-B=value", "A.B=value"] {
        let normalized = normalize_launch_command(vec![
            "env".to_string(),
            rejected.to_string(),
            "example-app".to_string(),
        ])
        .expect("invalid assignment becomes the wrapped command");

        assert_eq!(normalized.executable, rejected, "{rejected}");
        assert_eq!(normalized.arguments, ["example-app"], "{rejected}");
        assert!(normalized.environment.is_empty(), "{rejected}");
    }
}
