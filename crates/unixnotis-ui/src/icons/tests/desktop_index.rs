use super::*;

#[test]
fn executable_basename_handles_paths_quotes_and_field_codes() {
    assert_eq!(
        executable_basename("'/opt/Demo App/bin/demo-app' --open %U"),
        Some("demo-app".to_string())
    );
    assert_eq!(
        executable_basename("firefox %u"),
        Some("firefox".to_string())
    );
    assert_eq!(executable_basename(""), None);
    assert_eq!(executable_basename("'unterminated"), None);
}

#[test]
fn desktop_index_resolves_authenticated_executable_to_application_icon() {
    let mut index = DesktopIconIndex::default();

    index.add_executable("demo-app", "org.example.Demo");
    index.add_executable("DEMO-APP", "org.example.Demo");

    assert_eq!(
        index.icons_for("demo-app"),
        Some(vec!["org.example.Demo".to_string()])
    );
}

#[test]
fn desktop_index_normalizes_ids_and_removes_duplicate_icons() {
    let mut index = DesktopIconIndex::default();
    index.add_id(" Example.App.desktop ", "example-icon");
    index.add_id("example.app.desktop", "example-icon");

    assert_eq!(
        index.icons_for("EXAMPLE.APP"),
        Some(vec!["example-icon".to_string()])
    );
}

#[test]
fn desktop_index_rejects_empty_lookup_keys() {
    let index = DesktopIconIndex::default();

    assert_eq!(index.icons_for("  "), None);
}
