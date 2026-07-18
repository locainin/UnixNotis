//! Import planning helpers for preset bundles
//!
//! This module turns validated bundle files into one concrete write plan
//! so dry-run, apply, and backup logic all reason about the same target set

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::super::archive::BundleFile;
use super::super::super::filesystem::ensure_safe_target_path;
use super::super::super::pathing::relative_path_matches_exclusion;

#[derive(Debug)]
pub(in crate::preset) struct ImportPlan {
    // Ordered write list used by both dry-run and the real apply step
    pub(in crate::preset) items: Vec<ImportPlanItem>,
    // Files that do not exist yet under the live config root
    pub(in crate::preset) created: usize,
    // Files that already exist and need a backup before replacement
    pub(in crate::preset) overwritten: usize,
    // Bundle files skipped because --except kept the local copy in place
    pub(in crate::preset) excluded: usize,
}

#[derive(Debug)]
pub(in crate::preset) struct ImportPlanItem {
    // Bundle file contents plus the bundle-relative target path
    pub(in crate::preset) file: BundleFile,
    // Final on-disk target under the live config root
    pub(in crate::preset) target_path: PathBuf,
    // Real files need to be copied into the backup root before replacement
    pub(in crate::preset) overwrite_existing: bool,
}

pub(in crate::preset) fn build_import_plan(
    config_dir: &Path,
    bundle_files: Vec<BundleFile>,
    exclusions: &[PathBuf],
) -> Result<ImportPlan> {
    let mut excluded = 0usize;
    let mut items = Vec::new();

    for file in bundle_files {
        // Import exclusions keep selected local files untouched even when the bundle carries them
        if relative_path_matches_exclusion(&file.relative_path, exclusions) {
            excluded += 1;
            continue;
        }

        // Path checks happen before planning so later apply logic only sees trusted targets
        let target_path = ensure_safe_target_path(config_dir, &file.relative_path)?;
        let overwrite_existing = target_path.exists();
        if overwrite_existing {
            // Imports only replace regular files so directories and devices cannot be clobbered
            let metadata = fs::symlink_metadata(&target_path)
                .with_context(|| format!("inspect existing target {}", target_path.display()))?;
            if !metadata.is_file() {
                return Err(anyhow!(
                    "preset import refuses to overwrite a non-file path: {}",
                    target_path.display()
                ));
            }
        }

        items.push(ImportPlanItem {
            file,
            target_path,
            overwrite_existing,
        });
    }

    validate_no_file_ancestor_conflicts(&items)?;

    // These counters are derived once so dry-run and the real apply path report the same numbers
    let overwritten = items.iter().filter(|item| item.overwrite_existing).count();
    let created = items.len().saturating_sub(overwritten);

    Ok(ImportPlan {
        items,
        created,
        overwritten,
        excluded,
    })
}

fn validate_no_file_ancestor_conflicts(items: &[ImportPlanItem]) -> Result<()> {
    for (index, item) in items.iter().enumerate() {
        for other in items.iter().skip(index + 1) {
            let first = &item.file.relative_path;
            let second = &other.file.relative_path;
            if first.starts_with(second) || second.starts_with(first) {
                return Err(anyhow!(
                    "preset import contains file paths that cannot both exist: {} and {}",
                    first.display(),
                    second.display()
                ));
            }
        }
    }
    Ok(())
}
