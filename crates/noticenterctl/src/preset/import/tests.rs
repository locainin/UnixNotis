use super::{import_preset_into, import_preset_into_with_confirm};
use crate::preset::archive::write_bundle;
use crate::preset::config_root::{CollectedConfigFiles, PresetFileSource};
use crate::preset::export::export_preset_from;
use crate::preset::manifest::{PresetManifest, PresetManifestFile};
use anyhow::anyhow;
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

    fn write(&self, relative_path: &str, contents: &str) {
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
    let collected = CollectedConfigFiles {
        files: [
            ("config.toml", "config.toml"),
            ("panel.css", "panel.css"),
            ("popup.css", "popup.css"),
            ("widgets.css", "widgets.css"),
            ("media.css", "media.css"),
        ]
        .into_iter()
        .map(|(relative_path, source_path)| {
            let source_path = export_root.path.join(source_path);
            PresetFileSource {
                // Build the archive by hand so export-side root checks do not hide the import bug
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
    // Building the archive by hand keeps this test focused on import-side validation only
    let manifest = PresetManifest::new(
        "demo".to_string(),
        "2026-04-11T00:00:00Z".to_string(),
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
    write_bundle(&bundle_path, &manifest, &collected).expect("write bundle");
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

#[test]
fn import_rejects_outside_css_asset_refs_in_noninteractive_runs() {
    // Shared presets should not silently import CSS that reaches outside the config root for assets
    let export_root = TempDirGuard::new("external-css-asset-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write(
        "base.css",
        ".panel { background-image: url(\"../outside.png\"); }\n",
    );
    let bundle_path = export_root.path.join("demo.unixnotis");
    let collected = CollectedConfigFiles {
        files: [("config.toml", "config.toml"), ("base.css", "base.css")]
            .into_iter()
            .map(|(relative_path, source_path)| {
                let source_path = export_root.path.join(source_path);
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
        "2026-04-11T00:00:00Z".to_string(),
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
    write_bundle(&bundle_path, &manifest, &collected).expect("write bundle");

    let import_root = TempDirGuard::new("external-css-asset-import");
    // The injected rejector makes this test stable even when cargo test is attached to a tty
    let error = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        false,
        false,
        |_refs| {
            Err(anyhow!(
                "preset import found CSS asset references that leave the UnixNotis config directory or use remote URLs"
            ))
        },
        |_exec_content, _allow_exec| Ok(()),
    )
    .expect_err("reject outside css asset refs without confirmation");

    assert!(error.to_string().contains(
        "CSS asset references that leave the UnixNotis config directory or use remote URLs"
    ));
    assert!(!import_root.path.join("config.toml").exists());
}

#[test]
fn import_skips_css_asset_warning_for_excluded_stylesheet() {
    let export_root = TempDirGuard::new("excluded-css-warning-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write("base.css", ".panel { color: red; }\n");
    export_root.write(
        "assets.css",
        ".panel { background-image: url(\"../outside.png\"); }\n",
    );
    let bundle_path = export_root.path.join("demo.unixnotis");
    let collected = CollectedConfigFiles {
        files: [
            ("config.toml", "config.toml"),
            ("base.css", "base.css"),
            ("assets.css", "assets.css"),
        ]
        .into_iter()
        .map(|(relative_path, source_path)| {
            let source_path = export_root.path.join(source_path);
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
        "2026-04-16T00:00:00Z".to_string(),
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
    write_bundle(&bundle_path, &manifest, &collected).expect("write bundle");

    let import_root = TempDirGuard::new("excluded-css-warning-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &["assets.css".to_string()],
        false,
        false,
        |refs| {
            assert!(refs.is_empty());
            Ok(())
        },
        |_exec_content, _allow_exec| Ok(()),
    )
    .expect("ignore excluded stylesheet warning");

    assert_eq!(summary.file_count, 2);
    assert!(!import_root.path.join("assets.css").exists());
}

#[test]
fn import_rejects_command_bearing_bundle_by_default() {
    let export_root = TempDirGuard::new("command-bearing-export");
    export_root.write(
        "config.toml",
        r#"[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/probe.sh"
"#,
    );
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("command-bearing-import");
    let error = import_preset_into(&import_root.path, &bundle_path, &[], false)
        .expect_err("reject executable preset by default");

    assert!(error
        .to_string()
        .contains("rerun interactively to inspect them or use --allow-exec"));
    assert!(!import_root.path.join("config.toml").exists());
}

#[test]
fn import_skips_exec_review_for_excluded_script_payload() {
    let export_root = TempDirGuard::new("excluded-script-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("excluded-script-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &["scripts".to_string()],
        false,
        false,
        |_refs| Ok(()),
        |exec_content, allow_exec| {
            assert!(!allow_exec);
            assert!(exec_content.commands.is_empty());
            assert!(exec_content.files.is_empty());
            Ok(())
        },
    )
    .expect("ignore excluded script payload");

    assert_eq!(summary.file_count, 2);
    assert!(!import_root.path.join("scripts/probe.sh").exists());
}

#[test]
fn import_allows_command_bearing_bundle_when_explicitly_trusted() {
    let export_root = TempDirGuard::new("command-allowed-export");
    export_root.write(
        "config.toml",
        r#"[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/probe.sh"
"#,
    );
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("command-allowed-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        false,
        true,
        |_refs| Ok(()),
        |_exec_content, _allow_exec| Ok(()),
    )
    .expect("allow trusted executable preset");

    assert_eq!(summary.file_count, 3);
    assert!(import_root.path.join("scripts/probe.sh").exists());
}

#[test]
fn import_can_continue_after_exec_review_confirmation() {
    let export_root = TempDirGuard::new("command-reviewed-export");
    export_root.write(
        "config.toml",
        r#"[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/probe.sh"
"#,
    );
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("command-reviewed-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        false,
        false,
        |_refs| Ok(()),
        |exec_content, allow_exec| {
            assert!(!allow_exec);
            assert_eq!(exec_content.commands.len(), 1);
            assert_eq!(exec_content.files.len(), 1);
            Ok(())
        },
    )
    .expect("allow reviewed executable preset");

    assert_eq!(summary.file_count, 3);
    assert!(import_root.path.join("scripts/probe.sh").exists());
}
