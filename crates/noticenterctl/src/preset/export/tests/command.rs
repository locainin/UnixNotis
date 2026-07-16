use std::path::PathBuf;

use super::export_success_line;
use crate::preset::export::model::ExportSummary;

#[test]
fn export_success_line_reports_exact_file_count_and_bundle_path() {
    let summary = ExportSummary {
        bundle_path: PathBuf::from("portable.unixnotis"),
        file_count: 4,
        skipped_symlinks: Vec::new(),
        skipped_non_regular: Vec::new(),
    };

    assert_eq!(
        export_success_line(&summary),
        "preset export ok: 4 file(s) -> portable.unixnotis"
    );
}
