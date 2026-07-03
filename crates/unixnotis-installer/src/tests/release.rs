use super::{
    latest_tag_from_json, parse_version_tag, release_tag_is_newer, ReleaseStatus,
    ReleaseUpdateState,
};

#[test]
fn latest_tag_from_json_reads_tag_name() {
    let json = br#"{"tag_name":"v1.0.1","name":"UnixNotis v1.0.1"}"#;

    assert_eq!(latest_tag_from_json(json).expect("tag"), "v1.0.1");
}

#[test]
fn latest_tag_from_json_rejects_missing_tag() {
    let json = br#"{"name":"UnixNotis"}"#;

    assert!(latest_tag_from_json(json).is_err());
}

#[test]
fn release_tag_compare_detects_newer_patch_minor_and_major() {
    assert!(release_tag_is_newer("v1.0.1", "v1.0.0"));
    assert!(release_tag_is_newer("v1.1.0", "v1.0.9"));
    assert!(release_tag_is_newer("v2.0.0", "v1.9.9"));
    assert!(!release_tag_is_newer("v1.0.0", "v1.0.0"));
    assert!(!release_tag_is_newer("v0.9.9", "v1.0.0"));
}

#[test]
fn release_status_display_line_reports_available_update() {
    let status = ReleaseStatus {
        current: "v1.0.0".to_string(),
        latest: Some("v1.0.1".to_string()),
        state: ReleaseUpdateState::UpdateAvailable,
    };

    assert_eq!(status.display_line(), "v1.0.0 installed; v1.0.1 available");
}

#[test]
fn release_status_display_line_reports_up_to_date_release() {
    let status = ReleaseStatus {
        current: "v1.0.0".to_string(),
        latest: Some("v1.0.0".to_string()),
        state: ReleaseUpdateState::UpToDate,
    };

    assert_eq!(
        status.display_line(),
        "v1.0.0 installed; latest release is v1.0.0"
    );
}

#[test]
fn parse_version_tag_accepts_plain_and_prefixed_versions() {
    assert_eq!(parse_version_tag("1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_version_tag("v1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_version_tag("v1.2"), None);
    assert_eq!(parse_version_tag("v1.2.3.4"), None);
}
