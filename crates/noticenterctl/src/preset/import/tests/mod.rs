pub(super) use super::{import_preset_into, import_preset_into_with_confirm};
pub(super) use crate::preset::archive::write_bundle;
pub(super) use crate::preset::config_root::{CollectedConfigFiles, PresetFileSource};
pub(super) use crate::preset::export::export_preset_from;
pub(super) use crate::preset::manifest::{PresetManifest, PresetManifestFile};
pub(super) use anyhow::anyhow;
pub(super) use std::fs;
pub(super) use std::path::{Path, PathBuf};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod core;
mod css;
mod exec;

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(super) struct TempDirGuard {
    pub(super) path: PathBuf,
}

impl TempDirGuard {
    pub(super) fn new(name: &str) -> Self {
        // Unique temp roots keep import tests isolated from the real config tree
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "unixnotis-preset-import-{}-{}-{}",
            name, stamp, serial
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub(super) fn write(&self, relative_path: &str, contents: &str) {
        // Helper keeps test setup compact when building fake config roots
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, contents).expect("write file");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        // Best-effort cleanup keeps repeated test runs from piling up temp trees
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn write_collected_bundle(
    root: &TempDirGuard,
    bundle_path: &Path,
    stamp: &str,
    files: &[(&str, &str)],
) {
    // Hand-built bundles keep import tests focused when export-side checks would mask the real path
    let collected = CollectedConfigFiles {
        files: files
            .iter()
            .map(|(relative_path, source_path)| {
                let source_path = root.path.join(source_path);
                PresetFileSource {
                    relative_path: PathBuf::from(relative_path),
                    size: fs::metadata(&source_path).expect("metadata").len(),
                    source_path,
                    mode: 0o644,
                    contents_override: None,
                }
            })
            .collect(),
        ..Default::default()
    };
    let manifest = PresetManifest::new(
        "demo".to_string(),
        stamp.to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        collected
            .files
            .iter()
            .map(|file| PresetManifestFile {
                path: file.relative_path.display().to_string(),
                size: file.size,
            })
            .collect(),
    );
    write_bundle(bundle_path, &manifest, &collected).expect("write bundle");
}
