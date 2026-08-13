use std::fs;

use crate::{ThemeContractState, ThemeIncompatibility, ThemeManifest, THEME_API_VERSION};

use super::support::test_root;

fn theme_root(name: &str) -> std::path::PathBuf {
    let root = test_root(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temporary theme root should be creatable");
    root
}

fn theme_paths(root: &std::path::Path) -> crate::ThemePaths {
    crate::Config::default()
        .resolve_theme_paths_from(root)
        .expect("theme paths should resolve")
}

#[test]
fn missing_manifest_is_incompatible_for_export_review_without_creating_files() {
    let root = theme_root("theme-contract-stock");
    let paths = theme_paths(&root);

    let state = paths.inspect_theme_contract();

    assert_eq!(
        state,
        ThemeContractState::Incompatible(ThemeIncompatibility::MissingManifest)
    );
    assert!(!paths.manifest_path().exists());
    assert!(!paths.base_css.exists());
    fs::remove_dir_all(root).expect("temporary theme root should be removable");
}

#[test]
fn matching_manifest_enables_existing_custom_theme() {
    let root = theme_root("theme-contract-compatible");
    let paths = theme_paths(&root);
    fs::write(&paths.base_css, "/* custom */").expect("custom CSS should be writable");
    fs::write(
        paths.manifest_path(),
        format!("api_version = {THEME_API_VERSION}\nname = \"Night Glass\"\n"),
    )
    .expect("theme manifest should be writable");

    assert_eq!(
        paths.inspect_theme_contract(),
        ThemeContractState::Compatible(ThemeManifest {
            api_version: THEME_API_VERSION,
            name: "Night Glass".to_string(),
        })
    );
    fs::remove_dir_all(root).expect("temporary theme root should be removable");
}

#[test]
fn existing_theme_without_manifest_is_incompatible_and_unchanged() {
    let root = theme_root("theme-contract-missing");
    let paths = theme_paths(&root);
    let original = "/* preserve this exact theme */";
    fs::write(&paths.panel_css, original).expect("custom CSS should be writable");

    let state = paths.inspect_theme_contract();

    assert_eq!(
        state,
        ThemeContractState::Incompatible(ThemeIncompatibility::MissingManifest)
    );
    assert_eq!(
        fs::read_to_string(&paths.panel_css).expect("custom CSS should remain readable"),
        original
    );
    assert!(!paths.manifest_path().exists());
    fs::remove_dir_all(root).expect("temporary theme root should be removable");
}

#[test]
fn unsupported_manifest_version_falls_back_without_rewriting_theme() {
    let root = theme_root("theme-contract-version");
    let paths = theme_paths(&root);
    let original = "/* older theme */";
    fs::write(&paths.base_css, original).expect("custom CSS should be writable");
    fs::write(paths.manifest_path(), "api_version = 1\nname = \"Old\"\n")
        .expect("theme manifest should be writable");

    assert_eq!(
        paths.inspect_theme_contract(),
        ThemeContractState::Incompatible(ThemeIncompatibility::UnsupportedVersion { found: 1 })
    );
    assert_eq!(
        fs::read_to_string(&paths.base_css).expect("custom CSS should remain readable"),
        original
    );
    fs::remove_dir_all(root).expect("temporary theme root should be removable");
}

#[test]
fn blank_or_control_character_theme_names_are_incompatible() {
    let root = theme_root("theme-contract-invalid-names");
    let paths = theme_paths(&root);
    for name in ["   ", "Bad\\tName"] {
        fs::write(
            paths.manifest_path(),
            format!("api_version = {THEME_API_VERSION}\nname = \"{name}\"\n"),
        )
        .expect("theme manifest should be writable");

        assert_eq!(
            paths.inspect_theme_contract(),
            ThemeContractState::Incompatible(ThemeIncompatibility::InvalidName)
        );
    }
    fs::remove_dir_all(root).expect("temporary theme root should be removable");
}

#[test]
fn theme_name_length_accepts_the_limit_and_rejects_the_next_character() {
    let root = theme_root("theme-contract-name-limit");
    let paths = theme_paths(&root);
    let maximum_name = "a".repeat(128);
    fs::write(
        paths.manifest_path(),
        format!("api_version = {THEME_API_VERSION}\nname = \"{maximum_name}\"\n"),
    )
    .expect("theme manifest should be writable");
    assert!(matches!(
        paths.inspect_theme_contract(),
        ThemeContractState::Compatible(_)
    ));

    let oversized_name = "a".repeat(129);
    fs::write(
        paths.manifest_path(),
        format!("api_version = {THEME_API_VERSION}\nname = \"{oversized_name}\"\n"),
    )
    .expect("theme manifest should be writable");
    assert_eq!(
        paths.inspect_theme_contract(),
        ThemeContractState::Incompatible(ThemeIncompatibility::InvalidName)
    );
    fs::remove_dir_all(root).expect("temporary theme root should be removable");
}

#[cfg(unix)]
#[test]
fn linked_manifest_is_rejected_without_following_its_target() {
    use std::os::unix::fs::symlink;

    let root = theme_root("theme-contract-linked");
    let outside = theme_root("theme-contract-linked-outside");
    let paths = theme_paths(&root);
    fs::write(&paths.base_css, "/* custom */").expect("custom CSS should be writable");
    let target = outside.join("theme.toml");
    fs::write(&target, "api_version = 2\nname = \"Linked\"\n")
        .expect("outside manifest should be writable");
    symlink(&target, paths.manifest_path()).expect("manifest symlink should be creatable");

    assert_eq!(
        paths.inspect_theme_contract(),
        ThemeContractState::Incompatible(ThemeIncompatibility::UnreadableManifest)
    );
    assert_eq!(
        fs::read_to_string(target).expect("outside manifest should remain readable"),
        "api_version = 2\nname = \"Linked\"\n"
    );
    fs::remove_dir_all(root).expect("temporary theme root should be removable");
    fs::remove_dir_all(outside).expect("temporary outside root should be removable");
}
