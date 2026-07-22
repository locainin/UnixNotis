//! Tests for provisioning configured theme files

use std::fs;

use crate::{Config, DEFAULT_BASE_CSS};

use super::super::theme_files::warn_legacy_rename_once;
use super::support::test_root;

#[test]
fn ensure_theme_files_writes_missing_files_and_renames_legacy_style() {
    let root = test_root("theme-files");
    // Legacy style.css should be migrated only when base.css does not exist yet
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("theme root");
    fs::write(root.join("style.css"), "/* custom legacy */").expect("legacy css");

    let config = Config::default();
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    config
        .ensure_theme_files(&paths)
        .expect("theme files should be provisioned");

    // The legacy stylesheet becomes the new base stylesheet and leaves a backup marker
    assert_eq!(
        fs::read_to_string(&paths.base_css).expect("base css"),
        "/* custom legacy */"
    );
    assert!(paths.panel_css.exists());
    assert!(paths.popup_css.exists());
    assert!(paths.widgets_css.exists());
    assert!(paths.media_css.exists());
    assert!(root.join("style.css.bak").exists());
    assert!(!root.join("style.css").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_theme_files_preserves_existing_base_css() {
    let root = test_root("theme-preserve");
    // Existing base.css is user-owned and must win over legacy migration
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("theme root");

    let config = Config::default();
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.base_css, "/* keep */").expect("existing base css");
    fs::write(root.join("style.css"), "/* legacy ignored */").expect("legacy css");

    config
        .ensure_theme_files(&paths)
        .expect("theme files should be provisioned");

    assert_eq!(
        fs::read_to_string(&paths.base_css).expect("base css"),
        "/* keep */"
    );
    assert!(root.join("style.css").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_theme_files_keeps_legacy_style_when_backup_already_exists() {
    let root = test_root("theme-backup-exists");
    // A pre-existing backup means migration already happened or was handled by the user
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("theme root");
    fs::write(root.join("style.css"), "/* keep legacy */").expect("legacy css");
    fs::write(root.join("style.css.bak"), "/* keep backup */").expect("backup css");

    let config = Config::default();
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    config
        .ensure_theme_files(&paths)
        .expect("theme files should be provisioned");

    // Both legacy and backup files should remain untouched in this conservative path
    assert_eq!(
        fs::read_to_string(root.join("style.css")).expect("legacy css"),
        "/* keep legacy */"
    );
    assert_eq!(
        fs::read_to_string(root.join("style.css.bak")).expect("backup css"),
        "/* keep backup */"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn ensure_theme_files_ignores_a_legacy_symlink_and_keeps_its_target() {
    let root = test_root("theme-legacy-link");
    let protected = root.join("protected.css");
    let legacy = root.join("style.css");
    fs::create_dir_all(&root).expect("theme root");
    fs::write(&protected, "/* protected */").expect("protected css");
    std::os::unix::fs::symlink(&protected, &legacy).expect("legacy link");

    let config = Config::default();
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    config
        .ensure_theme_files(&paths)
        .expect("theme files should use defaults");

    assert_eq!(
        fs::read_to_string(&paths.base_css).expect("base css"),
        DEFAULT_BASE_CSS
    );
    assert_eq!(
        fs::read_to_string(&protected).expect("protected css"),
        "/* protected */"
    );
    assert!(fs::symlink_metadata(&legacy)
        .expect("legacy link remains")
        .file_type()
        .is_symlink());
    assert!(!root.join("style.css.bak").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn ensure_theme_files_preserves_a_dangling_backup_link_and_legacy_source() {
    let root = test_root("theme-dangling-backup-link");
    let legacy = root.join("style.css");
    let backup = root.join("style.css.bak");
    fs::create_dir_all(&root).expect("theme root");
    fs::write(&legacy, "/* legacy */").expect("legacy css");
    std::os::unix::fs::symlink("missing.css", &backup).expect("dangling backup link");

    let config = Config::default();
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    config
        .ensure_theme_files(&paths)
        .expect("theme files should be provisioned");

    assert_eq!(
        fs::read_to_string(&paths.base_css).expect("base css"),
        "/* legacy */"
    );
    assert_eq!(
        fs::read_to_string(&legacy).expect("legacy css"),
        "/* legacy */"
    );
    assert_eq!(
        fs::read_link(&backup).expect("backup link remains"),
        std::path::Path::new("missing.css")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_theme_files_ignores_an_oversized_legacy_theme() {
    const OVERSIZED_LEGACY_BYTES: usize = 16 * 1024 * 1024 + 1;

    let root = test_root("theme-oversized-legacy");
    let legacy = root.join("style.css");
    fs::create_dir_all(&root).expect("theme root");
    fs::write(&legacy, vec![b'x'; OVERSIZED_LEGACY_BYTES]).expect("oversized legacy css");

    let config = Config::default();
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    config
        .ensure_theme_files(&paths)
        .expect("theme files should use defaults");

    assert_eq!(
        fs::read_to_string(&paths.base_css).expect("base css"),
        DEFAULT_BASE_CSS
    );
    assert_eq!(
        fs::metadata(&legacy).expect("legacy css remains").len(),
        OVERSIZED_LEGACY_BYTES as u64
    );
    assert!(!root.join("style.css.bak").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_theme_files_accepts_a_legacy_theme_at_the_exact_size_limit() {
    const MAX_LEGACY_BYTES: usize = 16 * 1024 * 1024;

    let root = test_root("theme-exact-limit-legacy");
    let legacy = root.join("style.css");
    fs::create_dir_all(&root).expect("theme root");
    fs::write(&legacy, vec![b'x'; MAX_LEGACY_BYTES]).expect("limit-sized legacy css");

    let config = Config::default();
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    config
        .ensure_theme_files(&paths)
        .expect("limit-sized theme should migrate");

    assert_eq!(
        fs::metadata(&paths.base_css).expect("base css").len(),
        MAX_LEGACY_BYTES as u64
    );
    assert_eq!(
        fs::metadata(root.join("style.css.bak"))
            .expect("legacy backup")
            .len(),
        MAX_LEGACY_BYTES as u64
    );
    assert!(!legacy.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn legacy_rename_warning_is_emitted_only_once_per_process() {
    let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test failure");

    assert!(warn_legacy_rename_once(
        std::path::Path::new("style.css"),
        std::path::Path::new("style.css.bak"),
        &error,
    ));
    assert!(!warn_legacy_rename_once(
        std::path::Path::new("style.css"),
        std::path::Path::new("style.css.bak"),
        &error,
    ));
}
