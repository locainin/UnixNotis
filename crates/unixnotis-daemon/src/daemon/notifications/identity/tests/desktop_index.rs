use std::fs;

use super::*;
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
    let desktop = gio::DesktopAppInfo::from_filename(&path).expect("valid desktop entry");

    assert!(desktop_executable(&desktop).is_none());

    let mut index = DesktopIdentityIndex::default();
    index.add_desktop_file(&path, true);
    assert_eq!(index.records.len(), 1);
    assert!(index.records[0].executable_path.is_none());
    assert!(!index.records[0].system_entry);
}

#[test]
fn desktop_entry_exec_is_reduced_by_gio_to_its_program() {
    let root = TempRoot::new("desktop-with-exec");
    let path = root.join("org.example.True.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=True\nExec=/usr/bin/true %U\n",
    )
    .expect("desktop entry with Exec");
    let desktop = gio::DesktopAppInfo::from_filename(&path).expect("valid desktop entry");

    assert_eq!(
        desktop_executable(&desktop).as_deref(),
        Some(Path::new("/usr/bin/true"))
    );
}
