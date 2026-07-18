use crate::preset::import::command::summary::{summary_lines, ImportSummary};

#[test]
fn import_summary_reports_counts_without_inventing_a_backup() {
    let summary = ImportSummary {
        file_count: 4,
        created: 2,
        overwritten: 1,
        excluded: 1,
        backup_dir: None,
        dry_run: true,
    };

    let lines = summary_lines(&summary);

    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("4 file(s), 2 created, 1 overwritten, 1 excluded"));
    assert!(!lines[0].contains("backup"));
}
