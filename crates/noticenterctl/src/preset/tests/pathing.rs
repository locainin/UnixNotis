use std::path::{Path, PathBuf};

use super::super::pathing::{
    archive_payload_path, archive_payload_relative, normalize_relative_path, parse_except_paths,
    relative_path_matches_exclusion, resolve_cli_bundle_path_with_prompt, MANIFEST_ARCHIVE_PATH,
};

#[test]
fn parse_except_rejects_parent_traversal() {
    // Traversal should be blocked before any filesystem work starts
    let error = parse_except_paths(&["../escape".to_string()]).expect_err("reject traversal");
    assert!(error.to_string().contains("parent traversal"));
}

#[test]
fn normalize_relative_path_strips_dot_segments() {
    let normalized = normalize_relative_path(Path::new("./assets/bg.png")).expect("normalize path");
    assert_eq!(normalized, Path::new("assets/bg.png"));
}

#[test]
fn normalize_relative_path_rejects_parent_segments() {
    let error =
        normalize_relative_path(Path::new("./assets/../bg.png")).expect_err("reject parent");
    assert!(error.to_string().contains("parent traversal"));
}

#[test]
fn resolve_cli_bundle_path_appends_extension_after_confirmation() {
    // Missing extension should be fixable through the shared CLI path helper
    let resolved =
        resolve_cli_bundle_path_with_prompt(Path::new("dog"), |_original, _suggested| Ok(true))
            .expect("resolve preset path");
    assert_eq!(resolved, Path::new("dog.unixnotis"));
}

#[test]
fn resolve_cli_bundle_path_cancels_when_prompt_is_declined() {
    // Declining the prompt should cancel the command instead of guessing
    let error =
        resolve_cli_bundle_path_with_prompt(Path::new("dog"), |_original, _suggested| Ok(false))
            .expect_err("cancel preset path");
    assert!(error.to_string().contains("canceled"));
}

#[test]
fn archive_payload_round_trip_keeps_relative_payload_path() {
    let relative_path = Path::new("assets/bg.png");
    let archive_path = archive_payload_path(relative_path);
    let decoded = archive_payload_relative(&archive_path).expect("decode payload path");

    assert_eq!(decoded, Some(PathBuf::from(relative_path)));
}

#[test]
fn archive_payload_relative_ignores_manifest_entry() {
    let decoded =
        archive_payload_relative(Path::new(MANIFEST_ARCHIVE_PATH)).expect("decode manifest path");
    assert_eq!(decoded, None);
}

#[test]
fn relative_path_matches_exclusion_accepts_directory_prefixes() {
    assert!(relative_path_matches_exclusion(
        Path::new("scripts/install.sh"),
        &[PathBuf::from("scripts")]
    ));
    assert!(!relative_path_matches_exclusion(
        Path::new("assets/scripts/icon.png"),
        &[PathBuf::from("scripts")]
    ));
}
