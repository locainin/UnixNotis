use std::collections::HashMap;

use super::super::{add_icon_to_map, normalize_key, DesktopIconIndex};

#[test]
fn desktop_icon_index_returns_id_wm_class_and_name_matches_without_duplicates() {
    let mut index = DesktopIconIndex::default();
    index.add_name("Calendar", "calendar-icon");
    index.add_wm_class("calendar", "calendar-wm-icon");
    index.add_id("calendar.desktop", "calendar-id-icon");
    index.add_name("calendar", "calendar-icon");

    let icons = index.icons_for(" Calendar ").expect("icons");

    assert_eq!(
        icons,
        vec!["calendar-id-icon", "calendar-wm-icon", "calendar-icon"]
    );
    assert_eq!(
        index.icons_for("calendar.desktop").expect("desktop icons"),
        vec!["calendar-id-icon"]
    );
    assert!(index.icons_for("   ").is_none());
}

#[test]
fn add_icon_to_map_rejects_empty_inputs_and_dedupes_icons() {
    let mut map = HashMap::new();

    add_icon_to_map(&mut map, " App ", "icon-a");
    add_icon_to_map(&mut map, "app", "icon-a");
    add_icon_to_map(&mut map, "", "icon-b");
    add_icon_to_map(&mut map, "app", "");

    assert_eq!(
        map.get("app").expect("app icons"),
        &vec!["icon-a".to_string()]
    );
}

#[test]
fn normalize_key_trims_and_lowercases_lookup_keys() {
    assert_eq!(normalize_key("  Org.Example.App  "), "org.example.app");
}
