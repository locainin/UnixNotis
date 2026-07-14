//! Import summary data and reporting

use std::path::PathBuf;

use super::plan::ImportPlan;

#[derive(Debug)]
pub(super) struct ImportSummary {
    // Number of files that will be or were applied from the bundle
    pub(super) file_count: usize,
    // Files that did not exist locally before import
    pub(super) created: usize,
    // Files that already existed and needed a backup first
    pub(super) overwritten: usize,
    // Bundle files intentionally left untouched because of --except
    pub(super) excluded: usize,
    // Backup directory is present only when an overwrite happened
    pub(super) backup_dir: Option<PathBuf>,
    // Dry-run keeps the same output shape without touching the filesystem
    pub(super) dry_run: bool,
}

pub(super) const fn build_summary(
    plan: &ImportPlan,
    backup_dir: Option<PathBuf>,
    dry_run: bool,
) -> ImportSummary {
    ImportSummary {
        file_count: plan.items.len(),
        created: plan.created,
        overwritten: plan.overwritten,
        excluded: plan.excluded,
        backup_dir,
        dry_run,
    }
}

pub(super) fn print_summary(summary: &ImportSummary) -> Vec<String> {
    let lines = summary_lines(summary);
    for line in &lines {
        println!("{line}");
    }
    lines
}

pub(super) fn summary_lines(summary: &ImportSummary) -> Vec<String> {
    let mut lines = vec![format!(
        "preset import {}: {} file(s), {} created, {} overwritten, {} excluded",
        if summary.dry_run { "dry-run ok" } else { "ok" },
        summary.file_count,
        summary.created,
        summary.overwritten,
        summary.excluded
    )];
    if let Some(backup_dir) = summary.backup_dir.as_ref() {
        lines.push(format!("preset import backup: {}", backup_dir.display()));
    }
    lines
}
