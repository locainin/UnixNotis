use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::lint_geometry_css_files_with_config;
use unixnotis_core::Config;

#[test]
fn geometry_check_reads_valid_files_into_one_complete_report() {
    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-geometry-check-{}-{serial}",
        std::process::id(),
    ));
    let css = root.join("panel.css");
    fs::create_dir_all(&root).expect("create geometry test directory");
    fs::write(&css, ".unixnotis-panel { padding: 8px; }").expect("write geometry css");

    let diagnostics = lint_geometry_css_files_with_config(
        std::slice::from_ref(&css),
        &root,
        "$CONFIG",
        "$CONFIG/config.toml",
        &Config::default(),
    )
    .expect("valid CSS file should produce a report");

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.display_path != "$CONFIG/missing.css"));
    fs::remove_dir_all(root).expect("remove geometry test directory");
}
