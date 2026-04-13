use super::{
    collect_external_css_asset_refs_from_bundle, collect_external_css_asset_refs_from_paths,
};
use crate::preset::archive::BundleFile;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        // Unique temp roots keep the CSS asset tests isolated from export and import tests
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "unixnotis-css-asset-refs-{}-{}-{}",
            name, stamp, serial
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        // A small write helper keeps each test focused on the CSS behavior being checked
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, contents).expect("write file");
        path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn finds_file_url_outside_root_in_bundle_css() {
    // Bundle scanning should flag a local file URL that reaches outside the config root
    let root = TempDirGuard::new("bundle");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    let refs = collect_external_css_asset_refs_from_bundle(
        &config_dir,
        &[BundleFile {
            relative_path: PathBuf::from("base.css"),
            contents: b".panel { background-image: url(\"file:///tmp/outside.png\"); }\n".to_vec(),
            mode: 0o644,
        }],
    );

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reason, "local path points outside the config root");
}

#[test]
fn finds_relative_parent_escape_in_live_css() {
    // Live CSS scanning should still catch a relative asset path that walks out of the root
    let root = TempDirGuard::new("relative");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let css_path = root.write(
        "xdg/unixnotis/base.css",
        ".panel { background-image: url(\"../outside.png\"); }\n",
    );

    let refs =
        collect_external_css_asset_refs_from_paths(&config_dir, &[css_path]).expect("scan css");

    // The exact reason string matters because export and css-check print it back to the user
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reason, "relative path leaves the config root");
}
