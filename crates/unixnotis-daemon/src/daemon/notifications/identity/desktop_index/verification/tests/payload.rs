use std::path::Path;

use super::super::payload::{
    literal_file_identities_are_current, literal_file_matches, verify_protected_payload,
};
use super::super::MAX_PROCESS_ARGUMENTS;
use super::support::structured_command;
use crate::daemon::notifications::identity::desktop_index::model::{
    FieldCode, LaunchArgument, LaunchFailure, LaunchSpec, LaunchVerification, LiteralArgument,
    VerifiedLaunch,
};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::daemon::notifications::identity::sender::{CommandLineEvidence, CommandLineQuality};

#[test]
fn protected_payload_verification_requires_current_file_identity_and_fixed_arguments() {
    let shell = executable_evidence_for_path(Path::new("/usr/bin/sh")).expect("system shell");
    let payload = executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system payload");
    let other =
        executable_evidence_for_path(Path::new("/usr/bin/false")).expect("other system payload");
    let payload_argument = LiteralArgument {
        value: b"/usr/bin/true".to_vec(),
        file: Some(("/usr/bin/true".into(), payload.identity)),
    };
    let spec = LaunchSpec {
        declared_executable: shell.identity,
        runtime_executable: shell.identity,
        arguments: vec![
            LaunchArgument::Literal(payload_argument.clone()),
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

    assert!(literal_file_matches(&payload_argument, b"/usr/bin/true"));
    assert!(!literal_file_matches(&payload_argument, b"/usr/bin/false"));
    assert!(!literal_file_matches(&payload_argument, &[0xff]));
    assert!(literal_file_identities_are_current(&spec));

    let mut stale_spec = spec.clone();
    let LaunchArgument::Literal(stale_payload) = &mut stale_spec.arguments[0] else {
        panic!("payload fixture should remain literal");
    };
    stale_payload.file = Some(("/usr/bin/true".into(), other.identity));
    assert!(!literal_file_identities_are_current(&stale_spec));

    let verified = structured_command(&["/usr/bin/sh", "/usr/bin/true", "--fixed"]);
    assert_eq!(
        verify_protected_payload(&verified, &spec),
        LaunchVerification::Verified(VerifiedLaunch::ProtectedPayload)
    );

    let wrong_payload = structured_command(&["/usr/bin/sh", "/usr/bin/false", "--fixed"]);
    assert_eq!(
        verify_protected_payload(&wrong_payload, &spec),
        LaunchVerification::DefinitiveMismatch(LaunchFailure::ProtectedPayloadMismatch)
    );

    let missing_argument = structured_command(&["/usr/bin/sh", "/usr/bin/true"]);
    assert_eq!(
        verify_protected_payload(&missing_argument, &spec),
        LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch)
    );
}

#[test]
fn trusted_payload_cannot_be_used_as_a_decoy_argument() {
    let runtime = executable_evidence_for_path(Path::new("/usr/bin/sh")).expect("system runtime");
    let payload = executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system payload");
    let spec = LaunchSpec {
        declared_executable: runtime.identity,
        runtime_executable: runtime.identity,
        arguments: vec![LaunchArgument::Literal(LiteralArgument {
            value: b"/usr/bin/true".to_vec(),
            file: Some(("/usr/bin/true".into(), payload.identity)),
        })],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };
    let sender = structured_command(&["/usr/bin/sh", "/usr/bin/false", "/usr/bin/true"]);

    assert_eq!(
        verify_protected_payload(&sender, &spec),
        LaunchVerification::DefinitiveMismatch(LaunchFailure::ProtectedPayloadMismatch),
        "a protected file after the active payload must not authenticate the runtime"
    );
}

#[test]
fn variable_width_field_before_protected_payload_does_not_create_false_conflict() {
    let payload =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("protected payload");
    let spec = LaunchSpec {
        declared_executable: payload.identity,
        runtime_executable: payload.identity,
        arguments: vec![
            LaunchArgument::FieldCode(FieldCode::Files),
            LaunchArgument::Literal(LiteralArgument {
                value: b"/usr/bin/true".to_vec(),
                file: Some((Path::new("/usr/bin/true").to_path_buf(), payload.identity)),
            }),
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
    let sender = structured_command(&[
        "/usr/bin/true",
        "/tmp/one.txt",
        "/tmp/two.txt",
        "/usr/bin/true",
        "--unexpected",
    ]);

    assert_eq!(
        verify_protected_payload(&sender, &spec),
        LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch),
        "a matched protected payload must not become contradictory because a later option differs"
    );
}
#[test]
fn protected_payload_accepts_exactly_the_bounded_argument_limit() {
    let runtime = executable_evidence_for_path(Path::new("/usr/bin/sh")).expect("system runtime");
    let payload = executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system payload");
    let mut arguments = vec![LaunchArgument::Literal(LiteralArgument {
        value: b"/usr/bin/true".to_vec(),
        file: Some(("/usr/bin/true".into(), payload.identity)),
    })];
    arguments.extend((1..MAX_PROCESS_ARGUMENTS).map(|_| {
        LaunchArgument::Literal(LiteralArgument {
            value: b"--fixed".to_vec(),
            file: None,
        })
    }));
    let spec = LaunchSpec {
        declared_executable: runtime.identity,
        runtime_executable: runtime.identity,
        arguments,
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };
    let mut argv = vec![b"/usr/bin/sh".to_vec(), b"/usr/bin/true".to_vec()];
    argv.extend((1..MAX_PROCESS_ARGUMENTS).map(|_| b"--fixed".to_vec()));
    let command = CommandLineEvidence {
        argv,
        quality: CommandLineQuality::Structured,
    };

    assert_eq!(command.argv.len().saturating_sub(1), MAX_PROCESS_ARGUMENTS);
    assert_eq!(
        verify_protected_payload(&command, &spec),
        LaunchVerification::Verified(VerifiedLaunch::ProtectedPayload)
    );
}
