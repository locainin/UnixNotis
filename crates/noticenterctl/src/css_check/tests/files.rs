use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::super::files::{collect_css_files, display_config_root, format_display_path};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(name: &str) -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "unixnotis-css-check-{name}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp tree");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn collect_css_files_recurses_skips_backup_dirs_and_sorts_results() {
    let temp = TempTree::new("collect");
    fs::create_dir_all(temp.path().join("theme/nested")).expect("create nested");
    fs::create_dir_all(temp.path().join("Backup-2026-01-01")).expect("create backup");
    fs::write(temp.path().join("theme/b.css"), "").expect("write b css");
    fs::write(temp.path().join("theme/nested/a.CSS"), "").expect("write a css");
    fs::write(temp.path().join("theme/nested/readme.txt"), "").expect("write txt");
    fs::write(temp.path().join("Backup-2026-01-01/ignored.css"), "").expect("write backup");

    let files = collect_css_files(temp.path()).expect("collect css files");
    let names = files
        .iter()
        .map(|path| {
            path.strip_prefix(temp.path())
                .expect("relative path")
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["theme/b.css", "theme/nested/a.CSS"]);
}

#[test]
fn collect_css_files_rejects_missing_root_with_context() {
    let temp = TempTree::new("missing");
    let missing = temp.path().join("missing");

    let error = collect_css_files(&missing).expect_err("missing root should fail");

    assert!(error.to_string().contains("resolve config directory"));
}

#[test]
fn format_display_path_uses_config_root_for_nested_files_only() {
    let config_dir = PathBuf::from("/tmp/unixnotis-test-config");
    let display_root = "$TEST_CONFIG/unixnotis";

    assert_eq!(
        format_display_path(
            &config_dir,
            display_root,
            &config_dir.join("style/main.css")
        ),
        "$TEST_CONFIG/unixnotis/style/main.css"
    );
    assert_eq!(
        format_display_path(&config_dir, display_root, &config_dir),
        "$TEST_CONFIG/unixnotis"
    );
    assert_eq!(
        format_display_path(
            &config_dir,
            display_root,
            Path::new("/tmp/other/style/main.css")
        ),
        "/tmp/other/style/main.css"
    );
}

#[test]
fn display_config_root_falls_back_to_literal_path_for_nonstandard_roots() {
    let root = PathBuf::from("/tmp/unixnotis-nonstandard-root");

    assert_eq!(
        display_config_root(&root),
        "/tmp/unixnotis-nonstandard-root"
    );
}
