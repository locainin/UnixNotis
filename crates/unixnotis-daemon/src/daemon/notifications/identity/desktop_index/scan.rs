//! Bounded desktop-entry discovery and record construction

use std::path::{Path, PathBuf};

use tracing::debug;

use super::model::DesktopIdentityIndex;

const MAX_DESKTOP_RECORDS: usize = 8_192;
const MAX_DIRECTORIES_VISITED: usize = 4_096;
const MAX_ENTRIES_VISITED: usize = 65_536;
const MAX_DIRECTORY_DEPTH: usize = 16;
const MAX_DESKTOP_FILE_BYTES: u64 = 256 * 1024;

#[derive(Debug, Copy, Clone)]
pub(in crate::daemon::notifications::identity) struct ScanLimits {
    pub(super) records: usize,
    pub(super) directories: usize,
    pub(super) entries: usize,
    pub(super) depth: usize,
    pub(super) file_bytes: u64,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            records: MAX_DESKTOP_RECORDS,
            directories: MAX_DIRECTORIES_VISITED,
            entries: MAX_ENTRIES_VISITED,
            depth: MAX_DIRECTORY_DEPTH,
            file_bytes: MAX_DESKTOP_FILE_BYTES,
        }
    }
}

#[derive(Debug, Default)]
pub(in crate::daemon::notifications::identity) struct ScanBudget {
    pub(super) directories: usize,
    pub(super) entries: usize,
    pub(super) skipped_files: usize,
    pub(super) stopped_by: Option<&'static str>,
}

impl ScanBudget {
    fn stop(&mut self, reason: &'static str) {
        self.stopped_by.get_or_insert(reason);
    }

    const fn exhausted(&self) -> bool {
        self.stopped_by.is_some()
    }
}

impl DesktopIdentityIndex {
    #[must_use]
    pub(crate) fn new() -> Self {
        let mut index = Self::default();
        let limits = ScanLimits::default();
        let mut budget = ScanBudget::default();
        // User entries are scanned first while origin remains part of the security identity
        for (root, system_entry) in desktop_roots() {
            index.scan_root(&root, system_entry, &limits, &mut budget);
            if budget.exhausted() {
                break;
            }
        }
        if budget.exhausted() || budget.skipped_files != 0 {
            // One summary avoids log floods from attacker-controlled application trees
            debug!(
                stopped_by = budget.stopped_by.unwrap_or("none"),
                directories = budget.directories,
                entries = budget.entries,
                skipped_files = budget.skipped_files,
                "desktop application scan reached a safety limit"
            );
        }
        // Relay trust is tied to the installed file identity instead of its basename
        index.index_trusted_relay(Path::new("/usr/bin/notify-send"));
        index.index_trusted_relay(Path::new("/usr/local/bin/notify-send"));
        // Portal backends carry broker-verified application ids into desktop notifications
        for directory in [
            "/usr/lib",
            "/usr/libexec",
            "/usr/local/lib",
            "/usr/local/libexec",
        ] {
            index.index_trusted_portals_in(Path::new(directory));
        }
        index
    }

    pub(super) fn scan_root(
        &mut self,
        root: &Path,
        system_entry: bool,
        limits: &ScanLimits,
        budget: &mut ScanBudget,
    ) {
        // A bounded iterative walk avoids recursion and unlimited desktop-file growth
        let mut pending = vec![(root.to_path_buf(), 0_usize)];
        while let Some((directory, depth)) = pending.pop() {
            if budget.directories >= limits.directories {
                budget.stop("directory budget");
                return;
            }
            budget.directories += 1;
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries {
                if budget.entries >= limits.entries {
                    budget.stop("entry budget");
                    return;
                }
                budget.entries += 1;
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                let Ok(metadata) = path.symlink_metadata() else {
                    continue;
                };
                let file_type = metadata.file_type();
                if file_type.is_dir() {
                    if depth >= limits.depth {
                        budget.stop("directory depth");
                        return;
                    }
                    pending.push((path, depth + 1));
                    continue;
                }
                if !file_type.is_file()
                    || path.extension().and_then(|value| value.to_str()) != Some("desktop")
                {
                    continue;
                }
                if metadata.len() > limits.file_bytes {
                    budget.skipped_files += 1;
                    continue;
                }
                if self.records.len() >= limits.records {
                    budget.stop("record budget");
                    return;
                }
                self.add_desktop_file(&path, system_entry);
            }
        }
    }
}

pub(super) fn desktop_roots() -> Vec<(PathBuf, bool)> {
    let mut roots = Vec::new();
    // The user data root remains distinct because its entries are not system evidence
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
    {
        roots.push((data_home.join("applications"), false));
    }
    let data_dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    roots.extend(std::env::split_paths(&data_dirs).map(|root| (root.join("applications"), true)));
    roots
}
