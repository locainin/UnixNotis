use std::path::Path;

use super::super::contract::{
    field_value_matches, match_ordered_dedicated_contract, match_ordered_exec_contract,
    verify_dedicated,
};
use super::support::structured_command;
use crate::daemon::notifications::identity::desktop_index::model::{
    FieldCode, LaunchArgument, LaunchFailure, LaunchSpec, LaunchVerification, LiteralArgument,
    VerifiedLaunch,
};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::daemon::notifications::identity::sender::{CommandLineEvidence, CommandLineQuality};

#[test]
fn ordered_contract_preserves_repeated_literals_and_field_positions() {
    let identity = executable_evidence_for_path(Path::new("/usr/bin/true"))
        .expect("system executable")
        .identity;
    let spec = LaunchSpec {
        declared_executable: identity,
        runtime_executable: identity,
        arguments: vec![
            LaunchArgument::Literal(LiteralArgument {
                value: b"--mode".to_vec(),
                file: None,
            }),
            LaunchArgument::Literal(LiteralArgument {
                value: b"safe".to_vec(),
                file: None,
            }),
            LaunchArgument::Literal(LiteralArgument {
                value: b"--mode".to_vec(),
                file: None,
            }),
            LaunchArgument::FieldCode(FieldCode::Url),
        ],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };

    assert!(match_ordered_exec_contract(
        &spec,
        &[
            b"--mode".to_vec(),
            b"safe".to_vec(),
            b"--mode".to_vec(),
            b"https://example.invalid/item".to_vec(),
        ],
    ));
    assert!(!match_ordered_exec_contract(
        &spec,
        &[
            b"--mode".to_vec(),
            b"--mode".to_vec(),
            b"safe".to_vec(),
            b"https://example.invalid/item".to_vec(),
        ],
    ));
}

#[test]
fn dedicated_contract_does_not_accept_reordered_fixed_options() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        declared_executable: executable.identity,
        runtime_executable: executable.identity,
        arguments: vec![
            LaunchArgument::Literal(LiteralArgument {
                value: b"--first".to_vec(),
                file: None,
            }),
            LaunchArgument::Literal(LiteralArgument {
                value: b"--second".to_vec(),
                file: None,
            }),
        ],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };

    assert_eq!(
        verify_dedicated(
            &structured_command(&["/usr/bin/true", "--first", "--second"]),
            &spec,
        ),
        LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable)
    );
    assert_eq!(
        verify_dedicated(
            &structured_command(&["/usr/bin/true", "--second", "--first"]),
            &spec,
        ),
        LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch)
    );
    assert_eq!(
        verify_dedicated(
            &structured_command(&[
                "/usr/bin/true",
                "--display=x11",
                "--first",
                "--tray",
                "--second",
                "--verbose",
            ]),
            &spec,
        ),
        LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable)
    );
    assert_eq!(
        verify_dedicated(
            &structured_command(&[
                "/usr/bin/true",
                "--first",
                "/tmp/unexpected-payload",
                "--second",
            ]),
            &spec,
        ),
        LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch)
    );
}

#[test]
fn empty_dedicated_contract_rejects_positional_payload() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        declared_executable: executable.identity,
        runtime_executable: executable.identity,
        arguments: Vec::new(),
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };

    assert_eq!(
        verify_dedicated(
            &structured_command(&["/usr/bin/true", "/tmp/attacker-payload"]),
            &spec,
        ),
        LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch)
    );
}

#[test]
fn empty_contract_with_unstructured_argv_is_not_verified() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        declared_executable: executable.identity,
        runtime_executable: executable.identity,
        arguments: Vec::new(),
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };

    for quality in [
        CommandLineQuality::RewrittenProcessTitle,
        CommandLineQuality::Truncated,
        CommandLineQuality::Unavailable,
    ] {
        let command_line = CommandLineEvidence {
            argv: Vec::new(),
            quality,
        };

        assert_eq!(
            verify_dedicated(&command_line, &spec),
            LaunchVerification::InsufficientEvidence(LaunchFailure::EmptyContractNeedsCommandLine),
            "an empty contract with {quality:?} argv must stay non-authoritative"
        );
    }
}

#[test]
fn empty_contract_accepts_only_non_positional_switches() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        declared_executable: executable.identity,
        runtime_executable: executable.identity,
        arguments: Vec::new(),
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };

    for arguments in [
        vec!["/usr/bin/true"],
        vec!["/usr/bin/true", "--verbose"],
        vec!["/usr/bin/true", "--display=x11", "-q"],
    ] {
        assert_eq!(
            verify_dedicated(&structured_command(&arguments), &spec),
            LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
            "standalone switches should remain compatible: {arguments:?}"
        );
    }
    assert_eq!(
        verify_dedicated(
            &structured_command(&["/usr/bin/true", "--title", "untrusted-value"]),
            &spec,
        ),
        LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch),
        "a separate option value is positional without an ordered contract"
    );
}
#[test]
fn optional_icon_contract_preserves_its_flag_and_value_relationship() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        declared_executable: executable.identity,
        runtime_executable: executable.identity,
        arguments: vec![
            LaunchArgument::OptionalIcon {
                name: "example-icon".to_string(),
            },
            LaunchArgument::Literal(LiteralArgument {
                value: b"--fixed".to_vec(),
                file: None,
            }),
        ],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };

    assert_optional_icon_contract(match_ordered_exec_contract, &spec, "protected");
    assert_optional_icon_contract(match_ordered_dedicated_contract, &spec, "dedicated");
}

#[test]
fn field_values_reject_empty_options_and_malformed_urls() {
    assert!(!field_value_matches(FieldCode::File, b""));
    assert!(!field_value_matches(FieldCode::Files, b"--runtime-option"));
    assert!(field_value_matches(FieldCode::File, b"relative-file"));
    assert!(field_value_matches(
        FieldCode::Url,
        b"https://example.invalid/item"
    ));
    assert!(!field_value_matches(FieldCode::Urls, b"not a URL"));
    assert!(!field_value_matches(FieldCode::Url, &[0xff]));
}

type ContractMatcher = fn(&LaunchSpec, &[Vec<u8>]) -> bool;

fn assert_optional_icon_contract(matcher: ContractMatcher, spec: &LaunchSpec, label: &str) {
    for (actual, expected) in [
        (vec![b"--fixed".to_vec()], true),
        (
            vec![
                b"--icon".to_vec(),
                b"example-icon".to_vec(),
                b"--fixed".to_vec(),
            ],
            true,
        ),
        (
            vec![
                b"--badge".to_vec(),
                b"example-icon".to_vec(),
                b"--fixed".to_vec(),
            ],
            false,
        ),
        (
            vec![
                b"--icon".to_vec(),
                b"other-icon".to_vec(),
                b"--fixed".to_vec(),
            ],
            false,
        ),
        (vec![b"--icon".to_vec(), b"--fixed".to_vec()], false),
    ] {
        assert_eq!(
            matcher(spec, &actual),
            expected,
            "{label}: actual={actual:?}"
        );
    }
}
