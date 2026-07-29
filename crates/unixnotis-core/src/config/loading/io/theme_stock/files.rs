//! Bounded, identity-stable file operations for stock theme migration

use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::filesystem::{open_regular_file, regular_file_contents_equal, write_file_if_missing};

use super::super::ConfigError;
use super::model::FileSnapshot;
use super::{MAX_STOCK_PATH_COLLISIONS, MAX_STOCK_THEME_BYTES};

const STOCK_PREVIEW_TAG: &str = "unixnotis-stock";
const STOCK_KEEP_TAG: &str = "unixnotis-stock-kept";

pub(super) fn inspect_stock_file(path: &Path) -> io::Result<(FileSnapshot, Vec<u8>)> {
    // One retained descriptor binds metadata and bytes to the same regular file
    let mut file = open_regular_file(path)?;
    let before = file.metadata()?;
    if before.len() > MAX_STOCK_THEME_BYTES {
        return Err(size_limit_error());
    }

    let capacity = usize::try_from(before.len())
        .map_err(|_error| io::Error::new(io::ErrorKind::InvalidData, "theme size is invalid"))?;
    let mut contents = Vec::with_capacity(capacity);
    file.by_ref()
        .take(MAX_STOCK_THEME_BYTES.saturating_add(1))
        .read_to_end(&mut contents)?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > MAX_STOCK_THEME_BYTES {
        return Err(size_limit_error());
    }

    // Metadata drift means an editor won the read race and the result cannot be authoritative
    let after = file.metadata()?;
    let before_snapshot = snapshot_for_metadata(&before, &contents)?;
    let after_snapshot = snapshot_for_metadata(&after, &contents)?;
    if before_snapshot != after_snapshot {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "stock theme changed while it was being inspected",
        ));
    }
    Ok((after_snapshot, contents))
}

fn snapshot_for_metadata(
    metadata: &std::fs::Metadata,
    contents: &[u8],
) -> io::Result<FileSnapshot> {
    Ok(FileSnapshot {
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.len(),
        modified: metadata.modified()?,
        digest: blake3::hash(contents),
    })
}

pub(in crate::config::loading::io) fn stock_preview_path(
    path: &Path,
) -> Result<PathBuf, ConfigError> {
    tagged_sibling_path(
        path,
        &format!("{STOCK_PREVIEW_TAG}-{}", env!("CARGO_PKG_VERSION")),
    )
}

pub(super) fn stock_preview_candidates(path: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let base = stock_preview_path(path)?;
    Ok((0..=MAX_STOCK_PATH_COLLISIONS)
        .map(|suffix| collision_candidate(&base, suffix))
        .collect())
}

pub(super) fn stock_keep_marker_path(base_dir: &Path) -> PathBuf {
    base_dir.join(format!(".{STOCK_KEEP_TAG}-{}", env!("CARGO_PKG_VERSION")))
}

pub(super) fn stock_backup_path(path: &Path) -> Result<PathBuf, ConfigError> {
    tagged_sibling_path(
        path,
        &format!("unixnotis-stock-before-{}", env!("CARGO_PKG_VERSION")),
    )
    .map(|mut path| {
        let mut name = path.as_os_str().to_os_string();
        name.push(".bak");
        path = PathBuf::from(name);
        path
    })
}

pub(super) fn reserve_stock_backup(path: &Path, existing: &[u8]) -> Result<PathBuf, ConfigError> {
    let base = stock_backup_path(path)?;
    for suffix in 0..=MAX_STOCK_PATH_COLLISIONS {
        let candidate = collision_candidate(&base, suffix);
        match write_file_if_missing(&candidate, existing, 0o644) {
            Ok(true) => return Ok(candidate),
            Ok(false) => {
                // A matching prior backup makes an interrupted Apply safe to retry
                if regular_file_contents_equal(&candidate, existing, MAX_STOCK_THEME_BYTES)
                    .unwrap_or(false)
                {
                    return Ok(candidate);
                }
            }
            Err(_error) => {
                // A linked or raced candidate cannot prevent trying the bounded suffix set
            }
        }
    }
    Err(ConfigError::ReadFailed(
        "no collision-free stock theme backup path is available".to_string(),
    ))
}

pub(super) fn collision_candidate(base: &Path, suffix: u8) -> PathBuf {
    if suffix == 0 {
        return base.to_path_buf();
    }
    let mut name = base.as_os_str().to_os_string();
    name.push(format!(".{suffix}"));
    PathBuf::from(name)
}

fn tagged_sibling_path(path: &Path, tag: &str) -> Result<PathBuf, ConfigError> {
    let file_name = path.file_name().ok_or_else(|| {
        ConfigError::ReadFailed(format!("theme path has no file name: {}", path.display()))
    })?;
    let mut sibling_name = file_name.to_os_string();
    sibling_name.push(".");
    sibling_name.push(tag);
    Ok(path.with_file_name(sibling_name))
}

fn size_limit_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "stock theme exceeds migration size limit",
    )
}
