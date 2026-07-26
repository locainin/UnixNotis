//! Bounded desktop-entry discovery and record construction

use std::path::{Path, PathBuf};

use tracing::debug;

use super::model::DesktopIdentityIndex;

const MAX_DESKTOP_RECORDS: usize = 8_192;
const MAX_DIRECTORIES_VISITED: usize = 4_096;
const MAX_ENTRIES_VISITED: usize = 65_536;
const MAX_DIRECTORY_DEPTH: usize = 16;
const MAX_DESKTOP_FILE_BYTES: u64 = 256 * 1024;

pub struct DesktopIndexSnapshot {
    pub index: DesktopIdentityIndex,
    pub watched_directories: Vec<PathBuf>,
}

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
            // Each trust class gets half of every global budget
            records: MAX_DESKTOP_RECORDS / 2,
            directories: MAX_DIRECTORIES_VISITED / 2,
            entries: MAX_ENTRIES_VISITED / 2,
            depth: MAX_DIRECTORY_DEPTH,
            file_bytes: MAX_DESKTOP_FILE_BYTES,
        }
    }
}

#[derive(Debug, Default)]
pub(in crate::daemon::notifications::identity) struct ScanBudget {
    pub(super) records: usize,
    pub(super) directories: usize,
    pub(super) entries: usize,
    pub(super) skipped_files: usize,
    pub(super) stopped_by: Option<&'static str>,
    pub(super) visited_directories: Vec<PathBuf>,
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
    pub(crate) fn build_snapshot() -> DesktopIndexSnapshot {
        Self::build_with_roots(desktop_roots(), &ScanLimits::default())
    }

    pub(super) fn build_with_roots(
        roots: Vec<(PathBuf, bool)>,
        limits: &ScanLimits,
    ) -> DesktopIndexSnapshot {
        let mut index = Self::default();
        // User-controlled trees and protected trees receive independent resource budgets
        let mut user_budget = ScanBudget::default();
        let mut system_budget = ScanBudget::default();
        for (root, system_entry) in roots {
            let budget = if system_entry {
                &mut system_budget
            } else {
                &mut user_budget
            };
            // Exhausting one trust class must not prevent the other class from being indexed
            if !budget.exhausted() {
                index.scan_root(&root, system_entry, limits, budget);
            }
        }
        for (scope, budget) in [("user", &user_budget), ("system", &system_budget)] {
            if budget.exhausted() || budget.skipped_files != 0 {
                // One summary avoids log floods from attacker-controlled application trees
                debug!(
                    scope,
                    stopped_by = budget.stopped_by.unwrap_or("none"),
                    records = budget.records,
                    directories = budget.directories,
                    entries = budget.entries,
                    skipped_files = budget.skipped_files,
                    "desktop application scan reached a safety limit"
                );
            }
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
        let watched_directories = user_budget
            .visited_directories
            .into_iter()
            .chain(system_budget.visited_directories)
            .collect();
        DesktopIndexSnapshot {
            index,
            watched_directories,
        }
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
            // Only readable directories can contribute records or useful kernel watches
            budget.visited_directories.push(directory);
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
                if budget.records >= limits.records {
                    budget.stop("record budget");
                    return;
                }
                let records_before = self.records.len();
                self.add_desktop_file(&path, system_entry);
                budget.records += self.records.len().saturating_sub(records_before);
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
