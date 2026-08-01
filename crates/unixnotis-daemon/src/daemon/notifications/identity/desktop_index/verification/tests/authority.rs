use std::collections::HashSet;
use std::path::Path;

use super::super::authority::{
    classify_launch_authority, executable_contract_is_dedicated, is_protected_payload,
};
use super::super::verify_record_launch;
use super::support::{record_for_spec, structured_command, test_package};
use crate::daemon::notifications::identity::desktop_index::model::{
    DesktopIdentityIndex, DesktopRecord, FieldCode, LaunchArgument, LaunchAuthority, LaunchSpec,
    LaunchVerification, LiteralArgument, VerifiedLaunch,
};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;

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
fn dynamic_contract_without_shared_provenance_is_not_dedicated() {
    for field_code in [FieldCode::Files, FieldCode::Urls] {
        let executable =
            executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system executable");
        let spec = LaunchSpec {
            declared_executable: executable.identity,
            runtime_executable: executable.identity,
            arguments: vec![LaunchArgument::FieldCode(field_code)],
            environment: Vec::new(),
            wrappers: Vec::new(),
            package_launcher: None,
            literal_files_are_system_managed: true,
        };
        let record = DesktopRecord {
            id: "org.example.Runtime".to_string(),
            display_name: "Runtime application".to_string(),
            badge_icon: "runtime".to_string(),
            desktop_path: Some("/usr/share/applications/org.example.Runtime.desktop".into()),
            declared_executable_path: Some("/usr/bin/true".into()),
            declared_executable_identity: Some(executable.identity),
            runtime_executable_path: Some("/usr/bin/true".into()),
            runtime_executable_identity: Some(executable.identity),
            desktop_identity: None,
            desktop_provenance: test_package("runtime-desktop"),
            declared_executable_provenance: test_package("runtime"),
            runtime_executable_provenance: test_package("runtime"),
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
        declared_executable: executable.identity,
        runtime_executable: executable.identity,
        arguments: vec![
            LaunchArgument::Literal(LiteralArgument {
                value: b"--".to_vec(),
                file: None,
            }),
            LaunchArgument::FieldCode(FieldCode::Url),
        ],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };
    let record = DesktopRecord {
        id: "org.example.True".to_string(),
        display_name: "True".to_string(),
        badge_icon: "true".to_string(),
        desktop_path: Some("/usr/share/applications/org.example.True.desktop".into()),
        declared_executable_path: Some("/usr/bin/true".into()),
        declared_executable_identity: Some(executable.identity),
        runtime_executable_path: Some("/usr/bin/true".into()),
        runtime_executable_identity: Some(executable.identity),
        desktop_identity: Some(executable.identity),
        desktop_provenance: test_package("true"),
        declared_executable_provenance: test_package("true"),
        runtime_executable_provenance: test_package("true"),
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
        declared_executable: executable.identity,
        runtime_executable: executable.identity,
        arguments: vec![LaunchArgument::FieldCode(FieldCode::Files)],
        environment: Vec::new(),
        wrappers: Vec::new(),
        package_launcher: None,
        literal_files_are_system_managed: true,
    };
    let record = DesktopRecord {
        id: "org.example.Runtime".to_string(),
        display_name: "True".to_string(),
        badge_icon: "runtime".to_string(),
        desktop_path: Some("/usr/share/applications/org.example.Runtime.desktop".into()),
        declared_executable_path: Some("/usr/bin/true".into()),
        declared_executable_identity: Some(executable.identity),
        runtime_executable_path: Some("/usr/bin/true".into()),
        runtime_executable_identity: Some(executable.identity),
        desktop_identity: Some(executable.identity),
        desktop_provenance: test_package("runtime-frontend"),
        declared_executable_provenance: test_package("shared-runtime"),
        runtime_executable_provenance: test_package("shared-runtime"),
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
            declared_executable: executable.identity,
            runtime_executable: executable.identity,
            arguments: vec![LaunchArgument::FieldCode(field_code)],
            environment: Vec::new(),
            wrappers: Vec::new(),
            package_launcher: None,
            literal_files_are_system_managed: true,
        };
        let mut record = record_for_spec("org.example.Viewer", &spec);
        record.desktop_provenance = test_package("example-viewer");
        record.declared_executable_provenance = test_package("example-viewer");
        record.runtime_executable_provenance = test_package("example-viewer");
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
            declared_executable: executable.identity,
            runtime_executable: executable.identity,
            arguments,
            environment: Vec::new(),
            wrappers: Vec::new(),
            package_launcher: None,
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
