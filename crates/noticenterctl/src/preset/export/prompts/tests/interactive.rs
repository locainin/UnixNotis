use std::path::PathBuf;

use super::{confirm_export_external_css_refs, format_external_css_ref_lines};
use crate::preset::css_asset_refs::ExternalCssAssetRef;

#[test]
fn empty_external_reference_list_needs_no_terminal_confirmation() {
    confirm_export_external_css_refs(&[]).expect("portable export should continue");
}

#[test]
fn external_reference_lines_label_remote_urls_without_echoing_internal_terms() {
    let references = vec![ExternalCssAssetRef {
        css_file: PathBuf::from("panel.css"),
        asset_ref: "https://example.invalid/background.png".to_string(),
        reason: "remote url".to_string(),
    }];

    let lines = format_external_css_ref_lines(&references);

    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("remote URL"));
}

#[test]
fn external_reference_lines_remove_terminal_controls() {
    let references = vec![ExternalCssAssetRef {
        css_file: PathBuf::from("panel.css"),
        asset_ref: "https://example.invalid/\u{1b}[2Jimage.png".to_string(),
        reason: "remote url".to_string(),
    }];

    let lines = format_external_css_ref_lines(&references);

    assert_eq!(lines.len(), 1);
    assert!(!lines[0].contains('\u{1b}'));
}
