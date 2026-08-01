use std::collections::HashSet;
use std::path::Path;

use super::super::authority::classify_launch_authority;
use super::super::{verify_record_launch, verify_record_launch_with};
use super::support::{record_for_spec, structured_command, test_package};
use crate::daemon::notifications::identity::desktop_index::model::{
    DesktopIdentityIndex, DesktopRecord, FieldCode, LaunchArgument, LaunchAuthority, LaunchFailure,
    LaunchSpec, LaunchVerification, LaunchWrapper, LiteralArgument, PackageLauncherBinding,
    VerifiedLaunch,
};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::daemon::notifications::identity::sender::CommandLineEvidence;

#[test]
fn package_launcher_target_verifies_with_matching_runtime_and_ordered_contract() {
    let launcher =
        executable_evidence_for_path(Path::new("/usr/bin/false")).expect("system launcher");
    let runtime = executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system runtime");
    let binding = PackageLauncherBinding {
        launcher_path: "/usr/bin/false".into(),
        launcher_identity: launcher.identity,
        launcher_digest: [7; 32],
        target_path: "/usr/bin/true".into(),
        target_identity: runtime.identity,
    };
    let spec = LaunchSpec {
        declared_executable: launcher.identity,
        runtime_executable: runtime.identity,
        arguments: vec![
            LaunchArgument::Literal(LiteralArgument {
                value: b"--".to_vec(),
                file: None,
            }),
            LaunchArgument::FieldCode(FieldCode::Url),
        ],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: Some(binding),
        literal_files_are_system_managed: true,
    };
    let package = test_package("example-chat");
    let mut record = record_for_spec("org.example.Chat", &spec);
    record.declared_executable_path = Some("/usr/bin/false".into());
    record.declared_executable_identity = Some(launcher.identity);
    record.runtime_executable_path = Some("/usr/bin/true".into());
    record.runtime_executable_identity = Some(runtime.identity);
    record.desktop_provenance = package.clone();
    record.declared_executable_provenance = package.clone();
    record.runtime_executable_provenance = package;
    let mut index = DesktopIdentityIndex::default();
    index.index_record(record);
    let indexed = index
        .records_for_id("org.example.Chat")
        .into_iter()
        .next()
        .expect("launcher-backed application record");

    let verification = verify_record_launch_with(
        indexed,
        &index,
        runtime.identity,
        &structured_command(&[
            "/usr/bin/true",
            "--password-store=desktop",
            "--display=x11",
            "--",
        ]),
        |_| true,
    );

    assert_eq!(
        verification,
        LaunchVerification::Verified(VerifiedLaunch::PackageLauncherTarget)
    );
}

#[test]
fn package_launcher_target_requires_current_binding_and_structured_arguments() {
    let launcher =
        executable_evidence_for_path(Path::new("/usr/bin/false")).expect("system launcher");
    let runtime = executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system runtime");
    let spec = LaunchSpec {
        declared_executable: launcher.identity,
        runtime_executable: runtime.identity,
        arguments: vec![LaunchArgument::FieldCode(FieldCode::Url)],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: Some(PackageLauncherBinding {
            launcher_path: "/usr/bin/false".into(),
            launcher_identity: launcher.identity,
            launcher_digest: [3; 32],
            target_path: "/usr/bin/true".into(),
            target_identity: runtime.identity,
        }),
        literal_files_are_system_managed: true,
    };
    let mut record = record_for_spec("org.example.Chat", &spec);
    record.declared_executable_identity = Some(launcher.identity);
    let mut index = DesktopIdentityIndex::default();
    index.index_record(record);
    let indexed = index
        .records_for_id("org.example.Chat")
        .into_iter()
        .next()
        .expect("launcher-backed application record");

    assert_eq!(
        verify_record_launch_with(
            indexed,
            &index,
            runtime.identity,
            &structured_command(&["/usr/bin/true"]),
            |_| false,
        ),
        LaunchVerification::InsufficientEvidence(LaunchFailure::LauncherBindingChanged)
    );
    assert_eq!(
        verify_record_launch_with(
            indexed,
            &index,
            runtime.identity,
            &CommandLineEvidence::default(),
            |_| true,
        ),
        LaunchVerification::InsufficientEvidence(LaunchFailure::MissingCommandLine)
    );
}

#[test]
fn shared_launcher_target_does_not_merge_incompatible_application_families() {
    let launcher =
        executable_evidence_for_path(Path::new("/usr/bin/false")).expect("system launcher");
    let runtime = executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system runtime");
    let spec = LaunchSpec {
        declared_executable: launcher.identity,
        runtime_executable: runtime.identity,
        arguments: vec![LaunchArgument::FieldCode(FieldCode::Url)],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: Some(PackageLauncherBinding {
            launcher_path: "/usr/bin/false".into(),
            launcher_identity: launcher.identity,
            launcher_digest: [9; 32],
            target_path: "/usr/bin/true".into(),
            target_identity: runtime.identity,
        }),
        literal_files_are_system_managed: true,
    };
    let package = test_package("example-suite");
    let mut first = record_for_spec("org.example.First", &spec);
    let mut second = record_for_spec("org.example.Second", &spec);
    for record in [&mut first, &mut second] {
        record.desktop_provenance = package.clone();
        record.declared_executable_provenance = package.clone();
        record.runtime_executable_provenance = package.clone();
    }
    let mut index = DesktopIdentityIndex::default();
    index.index_record(first);
    index.index_record(second);
    let indexed = index
        .records_for_id("org.example.First")
        .into_iter()
        .next()
        .expect("first application record");

    assert_eq!(
        classify_launch_authority(indexed, &index, &spec),
        LaunchAuthority::DynamicOnly,
        "one package-owned runtime shared by unrelated families must remain non-authoritative"
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
            declared_executable: executable.identity,
            runtime_executable: executable.identity,
            arguments: Vec::new(),
            environment: std::iter::repeat_n((b"A".to_vec(), b"1".to_vec()), environment_count)
                .collect(),
            wrappers: std::iter::repeat_n(LaunchWrapper::Env, wrapper_count).collect(),
            package_launcher: None,
            literal_files_are_system_managed: true,
        };
        let record = DesktopRecord {
            id: "org.example.True".to_string(),
            display_name: "Boundary".to_string(),
            badge_icon: "boundary".to_string(),
            desktop_path: Some("/usr/share/applications/org.example.True.desktop".into()),
            declared_executable_path: Some("/usr/bin/true".into()),
            declared_executable_identity: Some(executable.identity),
            runtime_executable_path: Some("/usr/bin/true".into()),
            runtime_executable_identity: Some(executable.identity),
            desktop_identity: None,
            desktop_provenance: test_package("true"),
            declared_executable_provenance: test_package("true"),
            runtime_executable_provenance: test_package("true"),
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
