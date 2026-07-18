use std::path::PathBuf;

use super::ExportSummary;

#[test]
fn export_summary_keeps_skipped_path_categories_distinct() {
    let summary = ExportSummary {
        bundle_path: PathBuf::from("theme.unixnotis"),
        file_count: 2,
        skipped_symlinks: vec![PathBuf::from("linked.css")],
        skipped_non_regular: vec![PathBuf::from("runtime.sock")],
    };

    assert_eq!(summary.file_count, 2);
    assert_eq!(summary.skipped_symlinks, vec![PathBuf::from("linked.css")]);
    assert_eq!(
        summary.skipped_non_regular,
        vec![PathBuf::from("runtime.sock")]
    );
}
