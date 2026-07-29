//! Desktop-identifier validation tests

use super::super::validation::validate_desktop_id;

#[test]
fn desktop_id_validation_accepts_ids_and_rejects_paths_or_control_characters() {
    assert_eq!(
        validate_desktop_id("org.example.App.desktop").as_deref(),
        Some("org.example.App")
    );
    assert_eq!(validate_desktop_id("../example"), None);
    assert_eq!(validate_desktop_id("org.example.\nApp"), None);
    assert_eq!(validate_desktop_id("."), None);
    assert_eq!(validate_desktop_id(".desktop"), None);
    assert_eq!(
        validate_desktop_id(&"a".repeat(256)).map(|id| id.len()),
        Some(256)
    );
    assert_eq!(validate_desktop_id(&"a".repeat(257)), None);
}
