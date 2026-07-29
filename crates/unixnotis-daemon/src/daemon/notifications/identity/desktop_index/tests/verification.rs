use std::collections::HashSet;
use std::path::Path;

use super::{
    classify_launch_authority, executable_contract_is_dedicated, field_value_matches,
    is_protected_payload, literal_file_identities_are_current, literal_file_matches,
    match_ordered_dedicated_contract, match_ordered_exec_contract, verify_dedicated,
    verify_protected_payload, verify_record_launch, MAX_PROCESS_ARGUMENTS,
};
use crate::daemon::notifications::identity::desktop_index::model::{
    DesktopIdentityIndex, DesktopRecord, FieldCode, LaunchArgument, LaunchAuthority, LaunchFailure,
    LaunchSpec, LaunchVerification, LaunchWrapper, LiteralArgument, VerifiedLaunch,
};
use crate::daemon::notifications::identity::desktop_index::provenance::PackageProvider;
use crate::daemon::notifications::identity::desktop_index::InstallProvenance;
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::daemon::notifications::identity::sender::{CommandLineEvidence, CommandLineQuality};

fn is_dynamic_or_option(argument: &LaunchArgument) -> bool {
    match argument {
        LaunchArgument::FieldCode(_) | LaunchArgument::OptionalIcon { .. } => true,
        LaunchArgument::Literal(literal) => literal.value.starts_with(b"-"),
    }
}

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
fn trusted_payload_cannot_be_used_as_a_decoy_argument() {
    let runtime = executable_evidence_for_path(Path::new("/usr/bin/sh")).expect("system runtime");
    let payload = executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system payload");
    let spec = LaunchSpec {
        executable: runtime.identity,
        arguments: vec![LaunchArgument::Literal(LiteralArgument {
            value: b"/usr/bin/true".to_vec(),
            file: Some(("/usr/bin/true".into(), payload.identity)),
        })],
        environment: Vec::new(),
        wrappers: Vec::new(),
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
        executable: payload.identity,
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
fn ordered_contract_preserves_repeated_literals_and_field_positions() {
    let spec = LaunchSpec {
        executable: executable_evidence_for_path(Path::new("/usr/bin/true"))
            .expect("system executable")
            .identity,
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
        executable: executable.identity,
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
        executable: executable.identity,
        arguments: Vec::new(),
        environment: Vec::new(),
        wrappers: Vec::new(),
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
        executable: executable.identity,
        arguments: Vec::new(),
        environment: Vec::new(),
        wrappers: Vec::new(),
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
        executable: executable.identity,
        arguments: Vec::new(),
        environment: Vec::new(),
        wrappers: Vec::new(),
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
fn dynamic_contract_without_shared_provenance_is_not_dedicated() {
    for field_code in [FieldCode::Files, FieldCode::Urls] {
        let executable =
            executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
        let spec = LaunchSpec {
            executable: executable.identity,
            arguments: vec![LaunchArgument::FieldCode(field_code)],
            environment: Vec::new(),
            wrappers: Vec::new(),
            literal_files_are_system_managed: true,
        };
        let record = DesktopRecord {
            id: "org.example.Runtime".to_string(),
            display_name: "Runtime application".to_string(),
            badge_icon: "runtime".to_string(),
            desktop_path: Some("/usr/share/applications/org.example.Runtime.desktop".into()),
            executable_path: Some("/usr/bin/true".into()),
            executable_identity: Some(executable.identity),
            desktop_identity: None,
            desktop_provenance: test_package("runtime-desktop"),
            executable_provenance: test_package("runtime"),
            system_origin: true,
            system_association: true,
            association_eligible: true,
            launch_spec: Some(spec.clone()),
            names: HashSet::new(),
        };
        let mut index = DesktopIdentityIndex::default();
        index.index_record(record);
        let indexed = index
            .records_for_id("org.example.Runtime")
            .into_iter()
            .next()
            .expect("single indexed runtime");

        assert_eq!(
            classify_launch_authority(indexed, &index, &spec),
            LaunchAuthority::DynamicOnly,
            "a single dynamic record must remain non-authoritative for {field_code:?}"
        );
    }
}

#[test]
fn dedicated_system_application_accepts_dynamic_url_field() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        executable: executable.identity,
        arguments: vec![
            LaunchArgument::Literal(LiteralArgument {
                value: b"--".to_vec(),
                file: None,
            }),
            LaunchArgument::FieldCode(FieldCode::Url),
        ],
        environment: Vec::new(),
        wrappers: Vec::new(),
        literal_files_are_system_managed: true,
    };
    let record = DesktopRecord {
        id: "org.example.True".to_string(),
        display_name: "True".to_string(),
        badge_icon: "true".to_string(),
        desktop_path: Some("/usr/share/applications/org.example.True.desktop".into()),
        executable_path: Some("/usr/bin/true".into()),
        executable_identity: Some(executable.identity),
        desktop_identity: Some(executable.identity),
        desktop_provenance: test_package("true"),
        executable_provenance: test_package("true"),
        system_origin: true,
        system_association: true,
        association_eligible: true,
        launch_spec: Some(spec.clone()),
        names: HashSet::from(["true".to_string()]),
    };
    let mut index = DesktopIdentityIndex::default();
    index.index_record(record);
    let indexed = index
        .records_for_id("org.example.True")
        .into_iter()
        .next()
        .expect("dedicated application record");

    assert_eq!(
        classify_launch_authority(indexed, &index, &spec),
        LaunchAuthority::DedicatedExecutable,
        "normal URL arguments must not erase dedicated executable authority"
    );
}

#[test]
fn dynamic_runtime_requires_matching_immutable_installation_provenance() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        executable: executable.identity,
        arguments: vec![LaunchArgument::FieldCode(FieldCode::Files)],
        environment: Vec::new(),
        wrappers: Vec::new(),
        literal_files_are_system_managed: true,
    };
    let record = DesktopRecord {
        id: "org.example.Runtime".to_string(),
        display_name: "True".to_string(),
        badge_icon: "runtime".to_string(),
        desktop_path: Some("/usr/share/applications/org.example.Runtime.desktop".into()),
        executable_path: Some("/usr/bin/true".into()),
        executable_identity: Some(executable.identity),
        desktop_identity: Some(executable.identity),
        desktop_provenance: test_package("runtime-frontend"),
        executable_provenance: test_package("shared-runtime"),
        system_origin: true,
        system_association: true,
        association_eligible: true,
        launch_spec: Some(spec.clone()),
        names: HashSet::from(["true".to_string()]),
    };
    let mut index = DesktopIdentityIndex::default();
    index.index_record(record);
    let indexed = index
        .records_for_id("org.example.Runtime")
        .into_iter()
        .next()
        .expect("runtime application record");

    assert_eq!(
        classify_launch_authority(indexed, &index, &spec),
        LaunchAuthority::DynamicOnly,
        "a package-owned shared runtime must not inherit desktop application authority"
    );
}

#[test]
fn package_backed_file_applications_verify_percent_f_and_percent_capital_f() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    for (field_code, actual) in [
        (FieldCode::File, vec!["/usr/bin/true", "/tmp/image.png"]),
        (
            FieldCode::Files,
            vec!["/usr/bin/true", "/tmp/first.png", "/tmp/second.png"],
        ),
    ] {
        let spec = LaunchSpec {
            executable: executable.identity,
            arguments: vec![LaunchArgument::FieldCode(field_code)],
            environment: Vec::new(),
            wrappers: Vec::new(),
            literal_files_are_system_managed: true,
        };
        let mut record = record_for_spec("org.example.Viewer", &spec);
        record.desktop_provenance = test_package("example-viewer");
        record.executable_provenance = test_package("example-viewer");
        let mut index = DesktopIdentityIndex::default();
        index.index_record(record);
        let indexed = index
            .records_for_id("org.example.Viewer")
            .into_iter()
            .next()
            .expect("file application record");

        assert_eq!(
            classify_launch_authority(indexed, &index, &spec),
            LaunchAuthority::DedicatedExecutable,
            "immutable application ownership should support {field_code:?}"
        );
        assert_eq!(
            verify_record_launch(
                indexed,
                &index,
                executable.identity,
                &structured_command(&actual),
            ),
            LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
            "the ordered {field_code:?} contract should accept matching document arguments"
        );
    }
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
            id: "org.example.True".to_string(),
            display_name: "Boundary".to_string(),
            badge_icon: "boundary".to_string(),
            desktop_path: Some("/usr/share/applications/org.example.True.desktop".into()),
            executable_path: Some("/usr/bin/true".into()),
            executable_identity: Some(executable.identity),
            desktop_identity: None,
            desktop_provenance: test_package("true"),
            executable_provenance: test_package("true"),
            system_origin: true,
            system_association: true,
            association_eligible: true,
            launch_spec: Some(spec),
            names: HashSet::new(),
        };
        let mut index = DesktopIdentityIndex::default();
        index.index_record(record);
        let indexed = index
            .records_for_id("org.example.True")
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

#[test]
fn dedicated_authority_accepts_document_fields_but_rejects_unprotected_fixed_payloads() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    for (arguments, expected) in [
        (Vec::new(), true),
        (vec![LaunchArgument::FieldCode(FieldCode::Url)], true),
        (vec![LaunchArgument::FieldCode(FieldCode::File)], true),
        (
            vec![LaunchArgument::Literal(LiteralArgument {
                value: b"runtime-selected-payload".to_vec(),
                file: None,
            })],
            false,
        ),
    ] {
        let spec = LaunchSpec {
            executable: executable.identity,
            arguments,
            environment: Vec::new(),
            wrappers: Vec::new(),
            literal_files_are_system_managed: true,
        };
        let mut index = DesktopIdentityIndex::default();
        index.index_record(record_for_spec("org.example.True", &spec));
        let record = index
            .records_for_id("org.example.True")
            .into_iter()
            .next()
            .expect("indexed dedicated boundary record");

        assert_eq!(
            executable_contract_is_dedicated(record, &index, &spec),
            expected,
            "arguments={:?}",
            spec.arguments
        );
    }
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
        executable: runtime.identity,
        arguments,
        environment: Vec::new(),
        wrappers: Vec::new(),
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

#[test]
fn optional_icon_contract_preserves_its_flag_and_value_relationship() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
    let spec = LaunchSpec {
        executable: executable.identity,
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

fn record_for_spec(id: &str, spec: &LaunchSpec) -> DesktopRecord {
    DesktopRecord {
        id: id.to_string(),
        display_name: "Contract application".to_string(),
        badge_icon: "contract".to_string(),
        desktop_path: Some(format!("/usr/share/applications/{id}.desktop").into()),
        executable_path: Some("/usr/bin/true".into()),
        executable_identity: Some(spec.executable),
        desktop_identity: None,
        desktop_provenance: test_package(id),
        executable_provenance: test_package(id),
        system_origin: true,
        system_association: true,
        association_eligible: true,
        launch_spec: Some(spec.clone()),
        names: HashSet::new(),
    }
}

fn test_package(package_id: &str) -> InstallProvenance {
    InstallProvenance::Package {
        provider: PackageProvider::Pacman,
        package_id: package_id.to_string(),
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
