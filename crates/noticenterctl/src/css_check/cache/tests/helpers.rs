use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::super::{
    parse_diagnostic_for_test, validate_css_parse_files_with, CssParseReport, CssParseWorkItem,
};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(super) struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    pub(super) fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("unixnotis-css-cache-{}-{}-{}", name, stamp, serial));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub(super) fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, contents).expect("write file");
        path
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn pause_for_metadata_tick() {
    // Fast file swaps can land in the same timestamp slot on some filesystems
    std::thread::sleep(Duration::from_millis(5));
}

#[cfg(unix)]
pub(super) fn set_file_mtime(path: &Path, modified: SystemTime) {
    use rustix::fs::{utimensat, AtFlags, Timespec, Timestamps, CWD};

    let duration = modified
        .duration_since(UNIX_EPOCH)
        .expect("mtime before unix epoch");
    let timestamp = Timespec {
        tv_sec: duration.as_secs().try_into().expect("mtime seconds fit"),
        tv_nsec: duration.subsec_nanos().into(),
    };
    let times = Timestamps {
        last_access: timestamp,
        last_modification: timestamp,
    };

    // Keeping mtime identical forces the old fast identity to collide
    utimensat(CWD, path, &times, AtFlags::empty()).expect("set file mtime");
}

pub(super) fn parse_with_counter(
    invocations: &Cell<usize>,
    work_item: &CssParseWorkItem,
) -> anyhow::Result<Vec<super::super::CachedParseDiagnostic>> {
    invocations.set(invocations.get() + 1);
    let contents = fs::read_to_string(&work_item.load_path)?;
    if contents == "clean" {
        return Ok(Vec::new());
    }
    Ok(parse_diagnostic_for_test(contents))
}

pub(super) fn validate_with_counter(
    invocations: &Cell<usize>,
    css_files: &[PathBuf],
    config_dir: &Path,
    cache_path: &Path,
) -> anyhow::Result<CssParseReport> {
    validate_css_parse_files_with(
        css_files,
        config_dir,
        "$TMP/unixnotis",
        cache_path,
        |work_item| parse_with_counter(invocations, work_item),
    )
}
