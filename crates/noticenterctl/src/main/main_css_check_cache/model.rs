use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use super::super::main_css_check_report::CssCheckDiagnostic;

pub(in super::super) struct CssParseReport {
    // Fresh and cached parser findings end up in one flat report
    pub(in super::super) diagnostics: Vec<CssCheckDiagnostic>,
    // Keeping a count avoids walking the vector again at the call site
    pub(in super::super) error_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct CssParseWorkItem {
    // The configured path stays as the visible path in fresh and cached reports
    pub(in super::super) load_path: PathBuf,
    // Canonical paths keep aliases and symlinks on one cache key
    pub(in super::super) canonical_path: PathBuf,
    // Stable identity keeps stale success and stale failure entries out
    pub(in super::super) identity: CssFileIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in super::super) struct CssFileIdentity {
    // Size is the cheapest first-pass mismatch
    pub(in super::super) size: u64,
    // Nanosecond precision keeps quick edits from colliding as often
    pub(in super::super) modified_nanos: u128,
    #[cfg(unix)]
    // Device and inode separate same-named files after replace or retarget
    pub(in super::super) device: u64,
    #[cfg(unix)]
    pub(in super::super) inode: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in super::super) enum CachedDiagnosticSource {
    // Top-level findings should always render against the current logical input path
    TopLevel,
    // Imported files need their own stable path in the report
    Path(PathBuf),
    // GTK can occasionally report inline data instead of a file path
    Data,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in super::super) struct CachedParseDiagnostic {
    // Source identity is cached separately so the visible path can be rebuilt later
    pub(in super::super) source: CachedDiagnosticSource,
    pub(in super::super) line: Option<usize>,
    pub(in super::super) column: Option<usize>,
    pub(in super::super) message: String,
    pub(in super::super) hint: Option<String>,
}

impl CssFileIdentity {
    pub(in super::super) fn from_metadata(metadata: &fs::Metadata) -> Result<Self> {
        // Metadata errors should stay attached to the file handling path
        let modified = metadata
            .modified()
            .context("read css file modification time")?;
        let modified_nanos = modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            Ok(Self {
                size: metadata.len(),
                modified_nanos,
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }

        #[cfg(not(unix))]
        {
            Ok(Self {
                size: metadata.len(),
                modified_nanos,
            })
        }
    }
}
