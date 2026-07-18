use std::collections::HashMap;

use super::super::listener::is_relevant_media_change;

#[test]
fn relevant_media_change_detects_updates_and_invalidations() {
    let mut changed = HashMap::new();
    changed.insert("Metadata", zbus::zvariant::Value::from("track"));
    let no_invalidations: [&str; 0] = [];

    assert!(is_relevant_media_change(&changed, &no_invalidations));

    let no_changes: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
    assert!(is_relevant_media_change(&no_changes, &["CanPlay"]));
}

#[test]
fn relevant_media_change_ignores_unrelated_properties() {
    let mut changed = HashMap::new();
    changed.insert("Volume", zbus::zvariant::Value::from(0.5_f64));

    assert!(!is_relevant_media_change(&changed, &["Position"]));
}
