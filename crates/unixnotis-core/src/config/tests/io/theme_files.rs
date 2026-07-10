use std::fs;

use crate::Config;

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
    assert!(paths.overrides_css.exists());
    assert!(fs::read_to_string(&paths.overrides_css)
        .expect("overrides css")
        .contains("final theme overrides"));
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
