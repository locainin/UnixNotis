//! Runtime-binding provenance and normalization cases

use std::collections::HashSet;

use super::{discard_untrusted_launcher_binding, runtime_binding_is_valid};
use crate::daemon::notifications::identity::desktop_index::model::{
    DesktopRecord, LaunchSpec, PackageLauncherBinding,
};
use crate::daemon::notifications::identity::desktop_index::provenance::{
    InstallProvenance, PackageProvider,
};
use crate::daemon::notifications::identity::executable::FileIdentity;

#[test]
fn desktop_launcher_and_target_must_share_package() {
    let record = launcher_record("example-app", "example-app", "example-app");

    assert!(runtime_binding_is_valid(&record));
}

#[test]
fn launcher_target_from_different_package_is_rejected() {
    let record = launcher_record("example-app", "example-app", "other-runtime");

    assert!(!runtime_binding_is_valid(&record));
}

#[test]
fn same_package_without_literal_launcher_relation_is_not_retained() {
    let mut record = launcher_record("example-app", "example-app", "example-app");
    record
        .launch_spec
        .as_mut()
        .expect("launcher launch specification")
        .package_launcher = None;

    assert!(!runtime_binding_is_valid(&record));
    discard_untrusted_launcher_binding(&mut record);
    let spec = record
        .launch_spec
        .as_ref()
        .expect("normalized direct launch specification");
    assert!(spec.declared_executable.same_file(spec.runtime_executable));
    assert_eq!(
        record.declared_executable_path,
        record.runtime_executable_path
    );
}

#[test]
fn direct_runtime_path_must_match_declared_path() {
    let mut record = launcher_record("example-app", "example-app", "example-app");
    let spec = record
        .launch_spec
        .as_mut()
        .expect("launcher launch specification");
    spec.package_launcher = None;
    spec.runtime_executable = spec.declared_executable;
    record.runtime_executable_identity = record.declared_executable_identity;

    assert!(!runtime_binding_is_valid(&record));
}

#[test]
fn direct_runtime_identity_must_match_declared_identity() {
    let mut record = launcher_record("example-app", "example-app", "example-app");
    record
        .launch_spec
        .as_mut()
        .expect("launcher launch specification")
        .package_launcher = None;
    record.runtime_executable_path = record.declared_executable_path.clone();

    assert!(!runtime_binding_is_valid(&record));
}

fn launcher_record(
    desktop_package: &str,
    launcher_package: &str,
    runtime_package: &str,
) -> DesktopRecord {
    let launcher = identity(41);
    let runtime = identity(42);
    DesktopRecord {
        id: "org.example.App".to_string(),
        display_name: "Example App".to_string(),
        badge_icon: "example-app".to_string(),
        desktop_path: Some("/usr/share/applications/org.example.App.desktop".into()),
        declared_executable_path: Some("/usr/bin/example-app".into()),
        declared_executable_identity: Some(launcher),
        runtime_executable_path: Some("/usr/lib/example-app/runtime".into()),
        runtime_executable_identity: Some(runtime),
        desktop_identity: Some(identity(40)),
        desktop_provenance: package(desktop_package),
        declared_executable_provenance: package(launcher_package),
        runtime_executable_provenance: package(runtime_package),
        system_origin: true,
        system_association: false,
        association_eligible: true,
        launch_spec: Some(LaunchSpec {
            declared_executable: launcher,
            runtime_executable: runtime,
            arguments: Vec::new(),
            environment: Vec::new(),
            wrappers: Vec::new(),
            package_launcher: Some(PackageLauncherBinding {
                launcher_path: "/usr/bin/example-app".into(),
                launcher_identity: launcher,
                launcher_digest: [5; 32],
                target_path: "/usr/lib/example-app/runtime".into(),
                target_identity: runtime,
            }),
            literal_files_are_system_managed: true,
        }),
        names: HashSet::new(),
    }
}

fn identity(inode: u64) -> FileIdentity {
    FileIdentity {
        device: 1,
        inode,
        uid: 0,
        mode: 0o100_755,
    }
}

fn package(package_id: &str) -> InstallProvenance {
    InstallProvenance::Package {
        provider: PackageProvider::Pacman,
        package_id: package_id.to_string(),
    }
}
