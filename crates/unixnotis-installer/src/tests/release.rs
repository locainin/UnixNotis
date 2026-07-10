use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tests::fs::write_executable;

use super::{
    current_version_tag, fetch_latest_release_tag, latest_release_curl_args, latest_tag_from_json,
    parse_version_tag, release_tag_is_newer, ReleaseStatus, ReleaseUpdateState,
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
fn current_version_tag_uses_cargo_package_version_with_release_prefix() {
    assert_eq!(
        current_version_tag(),
        format!("v{}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn fetch_latest_release_tag_reads_tag_from_successful_curl_json() {
    let _lock = crate::tests::env::test_env_lock();
    let root = temp_dir("release-fetch-success");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake curl directory");
    write_executable(
        &fake_bin.join("curl"),
        "#!/bin/sh\nprintf '%s\\n' '{\"tag_name\":\"v9.9.9\"}'\n",
    );
    let _fake_tools = crate::system_tools::use_fake_tool_bin(&fake_bin);

    let latest = fetch_latest_release_tag().expect("fake curl should return release JSON");

    assert_eq!(latest, "v9.9.9");
}

#[test]
fn fetch_latest_release_tag_rejects_failed_curl_status() {
    let _lock = crate::tests::env::test_env_lock();
    let root = temp_dir("release-fetch-failure");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake curl directory");
    write_executable(&fake_bin.join("curl"), "#!/bin/sh\nexit 22\n");
    let _fake_tools = crate::system_tools::use_fake_tool_bin(&fake_bin);

    let err = fetch_latest_release_tag().expect_err("failed curl must not look like an update");

    assert!(err.contains("curl exited"));
}

#[test]
fn latest_release_curl_args_disable_local_config_and_bound_download_size() {
    let args = latest_release_curl_args();

    // -q must stay first or curl may load .curlrc before the request is built
    assert_eq!(args.first().copied(), Some("-q"));
    assert!(args.windows(2).any(|pair| pair == ["--proto", "=https"]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--max-filesize", "65536"]));
}

#[test]
fn release_tag_compare_detects_newer_patch_minor_and_major() {
    assert!(release_tag_is_newer("v1.0.1", "v1.0.0"));
    assert!(release_tag_is_newer("v1.1.0", "v1.0.9"));
    assert!(release_tag_is_newer("v2.0.0", "v1.9.9"));
    assert!(release_tag_is_newer("v1.2.3", "v1.2.3-beta.1"));
    assert!(release_tag_is_newer("v1.2.4-beta.1", "v1.2.3"));
    assert!(!release_tag_is_newer("v1.0.0", "v1.0.0"));
    assert!(!release_tag_is_newer("v0.9.9", "v1.0.0"));
    assert!(!release_tag_is_newer("v1.2.3-beta.1", "v1.2.3"));
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
    assert!(parse_version_tag("1.2.3").is_some());
    assert!(parse_version_tag("v1.2.3").is_some());
    assert!(parse_version_tag("v1.2.3-beta.1").is_some());
    assert!(parse_version_tag("v1.2.3+build.4").is_some());
    assert_eq!(parse_version_tag("v1.2"), None);
    assert_eq!(parse_version_tag("v1.2.3.4"), None);
    assert_eq!(parse_version_tag("not-a-version"), None);
}

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("unixnotis-{name}-{}-{unique}", std::process::id()))
}
