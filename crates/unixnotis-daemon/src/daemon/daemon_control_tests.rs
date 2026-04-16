use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

use super::auth::{build_trusted_control_snapshots, is_trusted_control_executable_path_in_dir};
use super::clear_all_signal_plan;
use super::sanitize::{normalize_inhibit_scope, sanitize_inhibit_reason};

fn temp_dir(label: &str) -> std::path::PathBuf {
    // Fresh temp dir per test
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("unixnotis-auth-{label}-{pid}-{nanos}"));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn rejects_unknown_or_untrusted_paths() {
    // Random path must fail
    let trusted_dir = temp_dir("rejects-unknown");
    let outsider = trusted_dir.join("python3");
    fs::write(&outsider, "#!/bin/sh\n").expect("write outsider");
    let snapshots = build_trusted_control_snapshots(&trusted_dir);

    assert!(!is_trusted_control_executable_path_in_dir(
        Path::new("/tmp/noticenterctl"),
        &trusted_dir,
        &snapshots,
    ));
    assert!(!is_trusted_control_executable_path_in_dir(
        &outsider,
        &trusted_dir,
        &snapshots,
    ));

    let _ = fs::remove_dir_all(trusted_dir);
}

#[test]
fn rejects_trusted_name_alias_suffixes() {
    // Lookalike names must fail
    let trusted_dir = temp_dir("rejects-alias");
    let alias = trusted_dir.join("noticenterctl.exe");
    fs::write(&alias, "#!/bin/sh\n").expect("write alias");
    let snapshots = build_trusted_control_snapshots(&trusted_dir);

    assert!(!is_trusted_control_executable_path_in_dir(
        &alias,
        &trusted_dir,
        &snapshots,
    ));

    let _ = fs::remove_dir_all(trusted_dir);
}

#[test]
fn accepts_trusted_sibling_binary_only() {
    // Only the real sibling file should pass
    let trusted_dir = temp_dir("accepts-sibling");
    let trusted = trusted_dir.join("noticenterctl");
    fs::write(&trusted, "#!/bin/sh\n").expect("write trusted sibling");
    let snapshots = build_trusted_control_snapshots(&trusted_dir);

    assert!(is_trusted_control_executable_path_in_dir(
        &trusted,
        &trusted_dir,
        &snapshots,
    ));

    let other_dir = temp_dir("other-sibling");
    let forged = other_dir.join("noticenterctl");
    fs::write(&forged, "#!/bin/sh\n").expect("write forged sibling");
    assert!(!is_trusted_control_executable_path_in_dir(
        &forged,
        &trusted_dir,
        &snapshots,
    ));

    // Replacing the file must break trust
    fs::write(&trusted, "#!/bin/sh\necho forged\n").expect("overwrite trusted sibling");
    assert!(!is_trusted_control_executable_path_in_dir(
        &trusted,
        &trusted_dir,
        &snapshots,
    ));

    let _ = fs::remove_dir_all(trusted_dir);
    let _ = fs::remove_dir_all(other_dir);
}

#[cfg(unix)]
#[test]
fn rejects_group_writable_trusted_binary() {
    use std::os::unix::fs::PermissionsExt;

    let trusted_dir = temp_dir("rejects-group-writable");
    let trusted = trusted_dir.join("noticenterctl");
    fs::write(&trusted, "#!/bin/sh\n").expect("write trusted sibling");
    let mut permissions = fs::metadata(&trusted).expect("metadata").permissions();
    permissions.set_mode(0o775);
    fs::set_permissions(&trusted, permissions).expect("set permissions");

    let snapshots = build_trusted_control_snapshots(&trusted_dir);
    assert!(!is_trusted_control_executable_path_in_dir(
        &trusted,
        &trusted_dir,
        &snapshots,
    ));

    let _ = fs::remove_dir_all(trusted_dir);
}

#[test]
fn sanitize_inhibit_reason_trims_and_bounds() {
    // Empty falls back
    assert_eq!(sanitize_inhibit_reason("   "), "manual");
    let long = format!("{}🙂", "a".repeat(512));
    let bounded = sanitize_inhibit_reason(&long);
    assert!(bounded.len() <= 256);
}

#[test]
fn normalize_inhibit_scope_accepts_supported_values() {
    // Only known scopes pass
    assert_eq!(
        normalize_inhibit_scope(INHIBIT_SCOPE_ALL).expect("scope"),
        0
    );
    assert_eq!(
        normalize_inhibit_scope(INHIBIT_SCOPE_POPUPS).expect("scope"),
        INHIBIT_SCOPE_POPUPS
    );
    assert!(normalize_inhibit_scope(2).is_err());
}

#[test]
fn clear_all_with_no_active_rows_still_invalidates_snapshot() {
    let plan = clear_all_signal_plan(&[]);

    // No live rows means there is nothing to close
    assert!(!plan.emit_close_signals);
    // Empty clear is still the escape hatch for stale client rows
    assert!(plan.emit_snapshot_invalidated);
    // State refresh still needs a chance to run
    assert!(plan.emit_state_changed);
}

#[test]
fn clear_all_with_active_rows_keeps_close_fanout_and_refresh() {
    let plan = clear_all_signal_plan(&[11, 12]);

    // Active rows still need the normal close signals
    assert!(plan.emit_close_signals);
    // Clients still need a full refresh after the clear
    assert!(plan.emit_snapshot_invalidated);
    assert!(plan.emit_state_changed);
}
