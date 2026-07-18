use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    discover_installed_binaries, extract_bins_from_metadata, legacy_binaries,
    parse_install_binaries_metadata, parse_release_manifest_binaries, resolve_install_binaries,
    resolve_install_binaries_best_effort, resolve_target_directory, CargoMetadata,
};
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;
use crate::test_support::env::EnvGuard;
use crate::test_support::fs::write_executable;

#[test]
fn parse_install_binaries_metadata_reads_entries() {
    // Metadata order is preserved because installer logs and plans should stay predictable
    let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-daemon", "noticenterctl", "unixnotis-daemon"]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert_eq!(
        binaries,
        vec!["unixnotis-daemon".to_string(), "noticenterctl".to_string()]
    );
}

#[test]
fn parse_install_binaries_metadata_handles_missing_table() {
    // Missing installer metadata means discovery can fall back to cargo metadata
    let input = r#"
[workspace]
members = ["crates/unixnotis-daemon"]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert!(binaries.is_empty());
}

#[test]
fn parse_install_binaries_metadata_handles_empty_entries() {
    // Blank entries are ignored rather than becoming invalid binary names
    let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-daemon", "  ", ""]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert_eq!(binaries, vec!["unixnotis-daemon".to_string()]);
}

#[test]
fn parse_install_binaries_metadata_keeps_first_duplicate_only() {
    // De-duplication keeps the first declaration so repeated names do not alter install order
    let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-popups", "unixnotis-daemon", "unixnotis-popups"]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert_eq!(
        binaries,
        vec![
            "unixnotis-popups".to_string(),
            "unixnotis-daemon".to_string()
        ]
    );
}

#[test]
fn parse_release_manifest_binaries_uses_release_archive_order() {
    let input = r#"
{
  "version": "1.0.0",
  "binaries": ["unixnotis-daemon", "noticenterctl", "unixnotis-daemon"]
}
"#;

    let binaries = parse_release_manifest_binaries(input).expect("release manifest");

    // Release archives install the exact bundled list once, preserving manifest order
    assert_eq!(
        binaries,
        vec!["unixnotis-daemon".to_string(), "noticenterctl".to_string()]
    );
}

#[test]
fn parse_release_manifest_binaries_rejects_unknown_and_path_shaped_names() {
    for name in [
        "unixnotis-extra",
        "../unixnotis-daemon",
        "/tmp/noticenterctl",
    ] {
        let input = format!(r#"{{"version":"1.0.0","binaries":["{name}"]}}"#);
        let error = parse_release_manifest_binaries(&input)
            .expect_err("unsupported release binary must fail");

        assert!(error.to_string().contains("binary"), "{name}");
    }
}

#[test]
fn release_resolution_does_not_compare_archive_names_with_cargo_metadata() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("release-resolution");
    let paths = test_paths(&root);
    write_release_manifest(&paths, &["noticenterctl"]);
    let fake_bin = write_fake_cargo(
        &root,
        r#"{"target_directory":"target","packages":[{"targets":[{"name":"unixnotis-daemon","kind":["bin"]}]}]}"#,
    );
    let _path = EnvGuard::set("PATH", &fake_bin);

    let binaries =
        resolve_install_binaries(&paths).expect("release manifest should be authoritative");

    assert_eq!(binaries, vec!["noticenterctl".to_string()]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_resolution_keeps_declared_names_when_cargo_has_no_binary_targets() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("workspace-empty-cargo-targets");
    let paths = test_paths(&root);
    write_workspace_manifest(&paths, Some(&["noticenterctl"]));
    let fake_bin = write_fake_cargo(&root, r#"{"target_directory":"target","packages":[]}"#);
    let _path = EnvGuard::set("PATH", &fake_bin);

    let binaries = resolve_install_binaries(&paths).expect("workspace list should remain usable");

    assert_eq!(binaries, vec!["noticenterctl".to_string()]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_resolution_accepts_declared_names_present_in_cargo_metadata() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("workspace-matching-cargo-targets");
    let paths = test_paths(&root);
    write_workspace_manifest(&paths, Some(&["noticenterctl"]));
    let fake_bin = write_fake_cargo(
        &root,
        r#"{"target_directory":"target","packages":[{"targets":[{"name":"noticenterctl","kind":["bin"]}]}]}"#,
    );
    let _path = EnvGuard::set("PATH", &fake_bin);

    let binaries = resolve_install_binaries(&paths).expect("matching target should be accepted");

    assert_eq!(binaries, vec!["noticenterctl".to_string()]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_resolution_rejects_declared_names_missing_from_cargo_metadata() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("workspace-missing-cargo-target");
    let paths = test_paths(&root);
    write_workspace_manifest(&paths, Some(&["noticenterctl"]));
    let fake_bin = write_fake_cargo(
        &root,
        r#"{"target_directory":"target","packages":[{"targets":[{"name":"unixnotis-daemon","kind":["bin"]}]}]}"#,
    );
    let _path = EnvGuard::set("PATH", &fake_bin);

    let error = resolve_install_binaries(&paths).expect_err("missing target should be rejected");

    assert!(error.to_string().contains("noticenterctl"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_resolution_rejects_an_empty_declared_and_discovered_set() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("workspace-no-binaries");
    let paths = test_paths(&root);
    write_workspace_manifest(&paths, None);
    let fake_bin = write_fake_cargo(&root, r#"{"target_directory":"target","packages":[]}"#);
    let _path = EnvGuard::set("PATH", &fake_bin);

    let error = resolve_install_binaries(&paths).expect_err("empty discovery must fail closed");

    assert!(error.to_string().contains("no installable binaries"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_resolution_uses_cargo_targets_when_the_declared_list_is_missing() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("workspace-cargo-fallback");
    let paths = test_paths(&root);
    write_workspace_manifest(&paths, None);
    let fake_bin = write_fake_cargo(
        &root,
        r#"{"target_directory":"build-output","packages":[{"targets":[{"name":"noticenterctl","kind":["bin"]}]}]}"#,
    );
    let _path = EnvGuard::set("PATH", &fake_bin);

    let binaries = resolve_install_binaries(&paths).expect("cargo targets should provide fallback");

    assert_eq!(binaries, vec!["noticenterctl".to_string()]);
    assert_eq!(
        resolve_target_directory(&paths).expect("cargo target directory"),
        PathBuf::from("build-output")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_target_directory_is_the_archive_root() {
    let root = test_root("release-target-directory");
    let paths = test_paths(&root);
    write_release_manifest(&paths, &["noticenterctl"]);

    assert_eq!(
        resolve_target_directory(&paths).expect("release target directory"),
        paths.repo_root
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn parse_release_manifest_binaries_rejects_missing_binary_list() {
    let err = parse_release_manifest_binaries(r#"{"version":"1.0.0"}"#)
        .expect_err("missing binaries should fail");

    // A release without an explicit binary list cannot be safely installed
    assert!(err.to_string().contains("release manifest"));
}

#[test]
fn extract_bins_from_cargo_metadata_skips_installer_binary() {
    let input = r#"
{
  "target_directory": "target",
  "packages": [
    {
      "targets": [
        { "name": "unixnotis-daemon", "kind": ["bin"] },
        { "name": "unixnotis-installer", "kind": ["bin"] },
        { "name": "unixnotis-core", "kind": ["lib"] }
      ]
    }
  ]
}
"#;
    let metadata: CargoMetadata = serde_json::from_str(input).expect("metadata");
    let binaries = extract_bins_from_metadata(&metadata);
    assert_eq!(binaries, vec!["unixnotis-daemon".to_string()]);
}

#[test]
fn extract_bins_from_cargo_metadata_sorts_bins_and_keeps_non_installer_targets() {
    let input = r#"
{
  "target_directory": "target",
  "packages": [
    {
      "targets": [
        { "name": "unixnotis-center", "kind": ["bin"] },
        { "name": "unixnotis-core", "kind": ["lib"] },
        { "name": "noticenterctl", "kind": ["bin"] },
        { "name": "unixnotis-popups", "kind": ["bin"] }
      ]
    }
  ]
}
"#;
    let metadata: CargoMetadata = serde_json::from_str(input).expect("metadata");

    let binaries = extract_bins_from_metadata(&metadata);

    // Cargo metadata order can vary by package layout, so install planning sorts names
    assert_eq!(
        binaries,
        vec![
            "noticenterctl".to_string(),
            "unixnotis-center".to_string(),
            "unixnotis-popups".to_string()
        ]
    );
}

#[test]
fn discover_installed_binaries_keeps_only_regular_unixnotis_tools() {
    let root = test_root("discover-installed-binaries");
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    fs::write(bin_dir.join("unixnotis-daemon"), "").expect("daemon binary");
    fs::write(bin_dir.join("unixnotis-center"), "").expect("center binary");
    fs::write(bin_dir.join("noticenterctl"), "").expect("control binary");
    fs::write(bin_dir.join("other-tool"), "").expect("unrelated binary");
    fs::create_dir(bin_dir.join("unixnotis-directory")).expect("directory entry");

    let paths = test_paths(&root);

    let binaries = discover_installed_binaries(&paths);

    // Uninstall fallback must not touch unrelated tools or directory-shaped paths
    assert_eq!(
        binaries,
        vec![
            "noticenterctl".to_string(),
            "unixnotis-center".to_string(),
            "unixnotis-daemon".to_string()
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn discover_installed_binaries_returns_empty_when_bin_dir_is_missing() {
    let root = test_root("missing-bin-dir");
    let paths = test_paths(&root);

    let binaries = discover_installed_binaries(&paths);

    // Missing install dirs should be treated as nothing discovered, not as an error
    assert!(binaries.is_empty());
}

#[test]
fn best_effort_resolution_uses_discovered_binaries_before_legacy_fallback() {
    let root = test_root("best-effort-discovered");
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    fs::write(bin_dir.join("unixnotis-popups"), "").expect("popup binary");
    fs::write(bin_dir.join("noticenterctl"), "").expect("control binary");

    let (binaries, warning) = resolve_install_binaries_best_effort(&test_paths(&root));

    // A broken repo should still uninstall exactly the files that are present
    assert_eq!(
        binaries,
        vec!["noticenterctl".to_string(), "unixnotis-popups".to_string()]
    );
    assert!(warning
        .as_deref()
        .is_some_and(|text| text.contains("workspace Cargo.toml")));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn best_effort_resolution_uses_legacy_names_when_nothing_can_be_discovered() {
    let root = test_root("best-effort-legacy");

    let (binaries, warning) = resolve_install_binaries_best_effort(&test_paths(&root));

    // Legacy names keep uninstall useful for older installs even when repo metadata is unavailable
    assert_eq!(binaries, legacy_binaries());
    assert!(warning.is_some());
}

#[test]
fn legacy_binaries_keep_full_installed_surface() {
    let binaries = legacy_binaries();

    // This list is the safety net for uninstalling older installs with no metadata
    assert_eq!(
        binaries,
        vec![
            "unixnotis-daemon".to_string(),
            "unixnotis-popups".to_string(),
            "unixnotis-center".to_string(),
            "unixnotis-css-validate".to_string(),
            "noticenterctl".to_string()
        ]
    );
}

fn test_paths(root: &std::path::Path) -> InstallPaths {
    InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("bin"),
        service: ServiceManager::systemd_user(root.join("systemd")),
    }
}

fn test_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!("unixnotis-{name}-{unique}"))
}

fn write_workspace_manifest(paths: &InstallPaths, binaries: Option<&[&str]>) {
    fs::create_dir_all(&paths.repo_root).expect("workspace root");
    let metadata = binaries.map_or_else(String::new, |names| {
        let names = names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!("\n[workspace.metadata.unixnotis.installer]\nbinaries = [{names}]\n")
    });
    fs::write(
        paths.repo_root.join("Cargo.toml"),
        format!("[workspace]\nmembers = []\n{metadata}"),
    )
    .expect("workspace manifest");
}

fn write_release_manifest(paths: &InstallPaths, binaries: &[&str]) {
    fs::create_dir_all(paths.release_binary_dir()).expect("release bin directory");
    let binaries = binaries
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        paths.release_manifest_path(),
        format!(r#"{{"binaries":[{binaries}]}}"#),
    )
    .expect("release manifest");
}

fn write_fake_cargo(root: &std::path::Path, metadata: &str) -> PathBuf {
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake cargo directory");
    write_executable(
        &fake_bin.join("cargo"),
        &format!("#!/bin/sh\nprintf '%s\\n' '{metadata}'\n"),
    );
    fake_bin
}
