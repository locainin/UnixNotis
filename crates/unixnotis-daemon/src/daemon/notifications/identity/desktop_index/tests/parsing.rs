use std::fs;

use super::super::model::{LaunchArgument, LaunchWrapper};
use super::super::DesktopIdentityIndex;
use crate::test_support::TempRoot;

#[test]
fn dbus_activated_desktop_entry_without_exec_has_no_executable() {
    let root = TempRoot::new("desktop-without-exec");
    let path = root.join("org.example.NoExec.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=No Exec\nDBusActivatable=true\n",
    )
    .expect("desktop entry without Exec");
    let mut index = DesktopIdentityIndex::default();
    index.add_desktop_file(&path, true);
    assert_eq!(index.records.len(), 1);
    assert!(index.records[0].runtime_executable_path.is_none());
    assert!(!index.records[0].system_association);
}

#[test]
fn desktop_entry_exec_is_resolved_to_its_application_program() {
    let root = TempRoot::new("desktop-with-exec");
    let path = root.join("org.example.True.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=True\nExec=/usr/bin/true %U\n",
    )
    .expect("desktop entry with Exec");
    let mut index = DesktopIdentityIndex::default();
    index.add_desktop_file(&path, true);

    assert_eq!(
        index.records[0]
            .runtime_executable_path
            .as_deref()
            .and_then(std::path::Path::file_name),
        Some(std::ffi::OsStr::new("true"))
    );
}

#[test]
fn env_wrapped_desktop_entry_indexes_the_wrapped_application() {
    let root = TempRoot::new("desktop-env-wrapper");
    let path = root.join("org.example.Wrapped.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=Wrapped\nExec=/usr/bin/env FEATURE=1 /usr/bin/true --fixed %u\n",
    )
    .expect("desktop entry with env wrapper");
    let mut index = DesktopIdentityIndex::default();

    index.add_desktop_file(&path, true);

    let record = &index.records[0];
    assert_eq!(
        record
            .runtime_executable_path
            .as_deref()
            .and_then(std::path::Path::file_name),
        Some(std::ffi::OsStr::new("true"))
    );
    let spec = record.launch_spec.as_ref().expect("normalized launch spec");
    assert_eq!(spec.wrappers, [LaunchWrapper::Env]);
    assert_eq!(spec.environment, [(b"FEATURE".to_vec(), b"1".to_vec())]);
    assert!(matches!(
        &spec.arguments[0],
        LaunchArgument::Literal(argument) if argument.value == b"--fixed"
    ));
}

#[test]
fn generic_name_is_an_association_alias_but_not_a_protected_brand() {
    let root = TempRoot::new("desktop-generic-name");
    let path = root.join("org.example.Browser.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=Example Browser\nGenericName=Web Browser\nExec=/usr/bin/true\n",
    )
    .expect("desktop entry with generic name");
    let mut index = DesktopIdentityIndex::default();

    index.add_desktop_file(&path, true);

    assert!(index.records[0].claim_matches("Web Browser"));
    assert!(index.claim_matches_system_app("Example Browser"));
    assert!(!index.claim_matches_system_app("Web Browser"));
}

#[test]
fn desktop_categories_mark_conversation_capable_applications() {
    let root = TempRoot::new("desktop-communication-category");
    let path = root.join("org.example.Messages.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=Messages\nCategories=Network;InstantMessaging;\nExec=/usr/bin/true\n",
    )
    .expect("desktop entry with communication category");
    let mut index = DesktopIdentityIndex::default();

    index.add_desktop_file(&path, true);

    assert!(index.desktop_id_has_communication_role("org.example.messages"));

    // A communication marker without an indexed desktop record is not enough evidence
    let mut role_only = DesktopIdentityIndex::default();
    role_only
        .communication_desktop_ids
        .insert("org.example.role-only".to_string());
    assert!(!role_only.desktop_id_has_communication_role("org.example.role-only"));

    // An indexed record without a communication category must not gain the role
    let mut record_only = DesktopIdentityIndex::default();
    record_only
        .by_id
        .insert("org.example.record-only".to_string(), vec![0]);
    assert!(!record_only.desktop_id_has_communication_role("org.example.record-only"));
}
