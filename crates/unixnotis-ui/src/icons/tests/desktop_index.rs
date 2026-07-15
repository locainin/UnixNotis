use super::*;

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
