use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    discover_installed_binaries, extract_bins_from_metadata, legacy_binaries,
    parse_install_binaries_metadata, resolve_install_binaries_best_effort, CargoMetadata,
};
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

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
