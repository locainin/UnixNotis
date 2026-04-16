use super::*;

#[test]
fn import_dry_run_reports_create_and_overwrite_counts() {
    // Dry-run should plan writes without changing the target tree
    let export_root = TempDirGuard::new("dry-run-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write("base.css", ".a { color: red; }");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("dry-run-import");
    import_root.write("config.toml", "old = true");
    let summary = import_preset_into(
        &import_root.path,
        &bundle_path,
        &["base.css".to_string()],
        true,
    )
    .expect("dry-run import");

    // Excluding base.css leaves only config.toml in the write plan, and it already exists locally
    assert_eq!(summary.file_count, 1);
    assert_eq!(summary.created, 0);
    assert_eq!(summary.overwritten, 1);
    assert_eq!(summary.excluded, 1);
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read config"),
        "old = true"
    );
}

#[test]
fn import_writes_files_and_creates_backup_for_overwrites() {
    // Real import should overwrite live files and keep a rollback copy
    let export_root = TempDirGuard::new("real-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write("base.css", ".a { color: red; }");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("real-import");
    import_root.write("config.toml", "old = true");
    let summary = import_preset_into(&import_root.path, &bundle_path, &[], false).expect("import");

    // The bundle creates base.css and replaces config.toml, so only config.toml needs a backup copy
    assert_eq!(summary.file_count, 2);
    assert_eq!(summary.created, 1);
    assert_eq!(summary.overwritten, 1);
    let backup_dir = summary.backup_dir.expect("backup dir");
    assert!(backup_dir.join("config.toml").exists());
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read config"),
        "[theme]\nbase_css = \"base.css\"\n"
    );
}

#[test]
fn import_rejects_bundled_absolute_theme_paths_before_writing() {
    // A hostile config should not make post-import setup create files outside the config root
    let export_root = TempDirGuard::new("absolute-theme-export");
    let outside_theme = export_root.path.join("outside-theme.css");
    export_root.write(
        "config.toml",
        &format!(
            // The bundle looks normal on the surface, but one theme slot points outside the root
            "[theme]\nbase_css = {:?}\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n",
            outside_theme.display().to_string()
        ),
    );
    export_root.write("panel.css", ".panel { color: red; }");
    export_root.write("popup.css", ".popup { color: blue; }");
    export_root.write("widgets.css", ".widgets { color: green; }");
    export_root.write("media.css", ".media { color: yellow; }");
    let bundle_path = export_root.path.join("demo.unixnotis");
    // Building the archive by hand keeps this test focused on import-side validation only
    write_collected_bundle(
        &export_root,
        &bundle_path,
        "2026-04-11T00:00:00Z",
        &[
            ("config.toml", "config.toml"),
            ("panel.css", "panel.css"),
            ("popup.css", "popup.css"),
            ("widgets.css", "widgets.css"),
            ("media.css", "media.css"),
        ],
    );
    let import_root = TempDirGuard::new("absolute-theme-import");

    let error = import_preset_into(&import_root.path, &bundle_path, &[], false)
        .expect_err("reject absolute bundled theme path");

    // Early rejection should leave both the outside target and the import root untouched
    assert!(error
        .to_string()
        .contains("tries to leave the UnixNotis config directory"));
    assert!(!outside_theme.exists());
    assert!(!import_root.path.join("config.toml").exists());
}

#[test]
fn import_rejects_kept_local_config_that_points_outside_root() {
    // Keeping the local config should not reopen the later theme-materialization escape path
    let export_root = TempDirGuard::new("kept-local-config-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write("base.css", ".a { color: red; }");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("kept-local-config-import");
    let outside_theme = import_root.path.with_file_name("outside-theme.css");
    import_root.write(
        "config.toml",
        &format!(
            "[theme]\nbase_css = {:?}\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n",
            outside_theme.display().to_string()
        ),
    );

    let error = import_preset_into(
        &import_root.path,
        &bundle_path,
        &["config.toml".to_string()],
        false,
    )
    .expect_err("reject kept local config escape");

    // The kept local config is the whole attack surface here, so the bundle must not write base.css
    assert!(error
        .to_string()
        .contains("tries to leave the UnixNotis config directory"));
    assert!(!outside_theme.exists());
    assert!(!import_root.path.join("base.css").exists());
}
