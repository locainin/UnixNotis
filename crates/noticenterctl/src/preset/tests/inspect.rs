use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::export::export_preset_from;
use super::super::inspect::inspect_preset_at;

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        // Unique temp roots keep inspect tests independent from export and import tests
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "unixnotis-preset-inspect-{}-{}-{}",
            name, stamp, serial
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) {
        // Helper keeps the test body focused on the reported output
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, contents).expect("write file");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn inspect_lists_bundle_metadata_and_commands() {
    // Inspect should expose the command-bearing parts of the shared config
    let root = TempDirGuard::new("report");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[widgets.volume]\nget_cmd = \"wpctl get-volume @DEFAULT_AUDIO_SINK@\"\n",
    );
    root.write("base.css", ".a { color: red; }");
    let bundle_path = root.path.join("demo.unixnotis");
    export_preset_from(&root.path, &bundle_path, &[], false).expect("export");

    let report = inspect_preset_at(&bundle_path).expect("inspect");

    assert!(report.contains("preset: demo"));
    assert!(report.contains("widgets.volume.get_cmd"));
    assert!(report.contains("command path warnings:"));
    assert!(report.contains("host-specific command paths:"));
    assert!(report.contains("file list:"));
    assert!(report.contains("config.toml"));
}
