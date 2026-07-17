use super::*;

use crate::preset::css_asset_refs::ExternalCssAssetRef;
use crate::preset::import::summary::{print_summary, summary_lines, ImportSummary};

#[test]
fn format_external_css_ref_lines_normalizes_remote_url_reason() {
    let refs = vec![ExternalCssAssetRef {
        css_file: PathBuf::from("theme/panel.css"),
        asset_ref: "https://example.test/image.png".to_string(),
        reason: "remote url".to_string(),
    }];

    let lines = super::super::prompts::format_external_css_ref_lines(&refs);

    assert_eq!(
        lines,
        vec!["  - theme/panel.css -> https://example.test/image.png (remote URL)"]
    );
}

#[test]
fn format_external_css_ref_lines_preserves_local_path_reason() {
    let refs = vec![ExternalCssAssetRef {
        css_file: PathBuf::from("base.css"),
        asset_ref: "../outside.png".to_string(),
        reason: "local path points outside the config root".to_string(),
    }];

    let lines = super::super::prompts::format_external_css_ref_lines(&refs);

    assert_eq!(
        lines,
        vec!["  - base.css -> ../outside.png (local path points outside the config root)"]
    );
}

#[test]
fn confirm_import_external_css_refs_errors_without_interactive_confirmation() {
    let refs = vec![ExternalCssAssetRef {
        css_file: PathBuf::from("base.css"),
        asset_ref: "../outside.png".to_string(),
        reason: "local path points outside the config root".to_string(),
    }];

    let error =
        super::super::prompts::confirm_import_external_css_refs_with_terminal_state(&refs, false)
            .expect_err("noninteractive import should reject external refs");

    assert!(error.to_string().contains(
        "CSS asset references that leave the UnixNotis config directory or use remote URLs"
    ));
}

#[test]
fn summary_lines_include_backup_only_when_present() {
    let dry_run = ImportSummary {
        file_count: 2,
        created: 1,
        overwritten: 1,
        excluded: 0,
        backup_dir: None,
        dry_run: true,
    };
    assert_eq!(
        summary_lines(&dry_run),
        vec!["preset import dry-run ok: 2 file(s), 1 created, 1 overwritten, 0 excluded"]
    );

    let committed = ImportSummary {
        file_count: 1,
        created: 0,
        overwritten: 1,
        excluded: 2,
        backup_dir: Some(PathBuf::from("Backup-2026")),
        dry_run: false,
    };
    assert_eq!(
        summary_lines(&committed),
        vec![
            "preset import ok: 1 file(s), 0 created, 1 overwritten, 2 excluded",
            "preset import backup: Backup-2026",
        ]
    );
}

#[test]
fn print_summary_returns_the_lines_it_prints() {
    let summary = ImportSummary {
        file_count: 3,
        created: 2,
        overwritten: 1,
        excluded: 4,
        backup_dir: Some(PathBuf::from("Backup-printed")),
        dry_run: false,
    };

    assert_eq!(
        print_summary(&summary),
        vec![
            "preset import ok: 3 file(s), 2 created, 1 overwritten, 4 excluded",
            "preset import backup: Backup-printed",
        ]
    );
}
