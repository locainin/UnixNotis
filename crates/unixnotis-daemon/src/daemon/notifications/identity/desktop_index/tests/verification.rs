use std::collections::HashSet;
use std::path::Path;

use super::{
    is_dynamic_or_option, is_protected_payload, literal_file_identities_are_current,
    literal_file_matches, verify_protected_payload, verify_record_launch,
};
use crate::daemon::notifications::identity::desktop_index::model::{
    DesktopIdentityIndex, DesktopRecord, FieldCode, LaunchArgument, LaunchFailure, LaunchSpec,
    LaunchVerification, LaunchWrapper, LiteralArgument, VerifiedLaunch,
};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::daemon::notifications::identity::sender::{CommandLineEvidence, CommandLineQuality};

#[test]
fn authority_helpers_distinguish_dynamic_values_options_and_payloads() {
    let dynamic = LaunchArgument::FieldCode(FieldCode::Files);
    let option = LaunchArgument::Literal(LiteralArgument {
        value: b"--fixed".to_vec(),
        file: None,
    });
    let payload = LaunchArgument::Literal(LiteralArgument {
        value: b"/usr/share/example/app.bundle".to_vec(),
        file: Some((
            "/usr/share/example/app.bundle".into(),
            crate::daemon::notifications::identity::FileIdentity {
                device: 1,
                inode: 2,
                uid: 0,
                mode: 0o100_755,
            },
        )),
    });

    assert!(is_dynamic_or_option(&dynamic));
    assert!(is_dynamic_or_option(&option));
    assert!(!is_dynamic_or_option(&payload));
    assert!(!is_protected_payload(&dynamic));
    assert!(is_protected_payload(&payload));
}

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
        executable: shell.identity,
        arguments: vec![
            LaunchArgument::Literal(payload_argument.clone()),
            LaunchArgument::Literal(LiteralArgument {
                value: b"--fixed".to_vec(),
                file: None,
            }),
        ],
        environment: Vec::new(),
        wrappers: Vec::new(),
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
fn launch_verification_enforces_wrapper_and_environment_limits_at_the_boundary() {
    for (wrapper_count, environment_count, expected) in [
        (
            16,
            0,
            LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
        ),
        (
            17,
            0,
            LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper),
        ),
        (
            0,
            128,
            LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
        ),
        (
            0,
            129,
            LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper),
        ),
    ] {
        let executable =
            executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
        let spec = LaunchSpec {
            executable: executable.identity,
            arguments: Vec::new(),
            environment: std::iter::repeat_n((b"A".to_vec(), b"1".to_vec()), environment_count)
                .collect(),
            wrappers: std::iter::repeat_n(LaunchWrapper::Env, wrapper_count).collect(),
            literal_files_are_system_managed: true,
        };
        let record = DesktopRecord {
            id: "org.example.Boundary".to_string(),
            display_name: "Boundary".to_string(),
            badge_icon: "boundary".to_string(),
            executable_path: Some("/usr/bin/true".into()),
            executable_identity: Some(executable.identity),
            desktop_identity: None,
            system_origin: true,
            system_association: true,
            association_eligible: true,
            dbus_activatable: false,
            launch_spec: Some(spec),
            names: HashSet::new(),
        };
        let mut index = DesktopIdentityIndex::default();
        index.index_record(record);
        let indexed = index
            .records_for_id("org.example.Boundary")
            .into_iter()
            .next()
            .expect("indexed boundary record");

        assert_eq!(
            verify_record_launch(
                indexed,
                &index,
                executable.identity,
                &structured_command(&["/usr/bin/true"]),
            ),
            expected,
            "wrapper_count={wrapper_count}, environment_count={environment_count}"
        );
    }
}

fn structured_command(arguments: &[&str]) -> CommandLineEvidence {
    CommandLineEvidence {
        argv: arguments
            .iter()
            .map(|argument| argument.as_bytes().to_vec())
            .collect(),
        quality: CommandLineQuality::Structured,
    }
}
