//! Generated asset insertion and transformed bundle limits

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::preset::archive::{
    BundleFile, MAX_PRESET_PAYLOAD_FILES, MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};
use crate::preset::pathing::relative_path_matches_exclusion;

pub(super) fn append_generated_assets(
    files: &mut Vec<BundleFile>,
    exclusions: &[PathBuf],
    generated: BTreeMap<PathBuf, Vec<u8>>,
) -> Result<()> {
    for (relative_path, contents) in generated {
        if relative_path_matches_exclusion(&relative_path, exclusions) {
            return Err(anyhow!(
                "--except cannot remove a validated CSS image required by an imported stylesheet: {}",
                relative_path.display()
            ));
        }
        if let Some(existing) = files
            .iter()
            .find(|file| file.relative_path == relative_path)
        {
            if existing.contents != contents || existing.mode & 0o111 != 0 {
                return Err(anyhow!(
                    "preset file conflicts with a validated CSS image path: {}",
                    relative_path.display()
                ));
            }
            continue;
        }
        files.push(BundleFile {
            relative_path,
            contents,
            mode: 0o644,
        });
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(())
}

pub(super) fn validate_transformed_bundle(files: &[BundleFile]) -> Result<()> {
    let total_bytes = files.iter().try_fold(0u64, |total, file| {
        total
            .checked_add(file.contents.len() as u64)
            .ok_or_else(|| anyhow!("validated preset payload size overflow"))
    })?;
    validate_transformed_bundle_limits(files.len(), total_bytes)
}

pub(super) fn validate_transformed_bundle_limits(
    file_count: usize,
    total_bytes: u64,
) -> Result<()> {
    if file_count > MAX_PRESET_PAYLOAD_FILES {
        return Err(anyhow!(
            "validated CSS images raise the preset above {MAX_PRESET_PAYLOAD_FILES} files"
        ));
    }
    if total_bytes > MAX_PRESET_TOTAL_PAYLOAD_BYTES {
        return Err(anyhow!(
            "validated CSS images raise the preset above {MAX_PRESET_TOTAL_PAYLOAD_BYTES} bytes"
        ));
    }
    Ok(())
}
