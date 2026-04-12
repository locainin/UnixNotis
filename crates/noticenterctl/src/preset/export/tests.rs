use super::{export_preset_from, export_preset_from_with_confirm};
use crate::preset::archive::read_bundle;
use anyhow::anyhow;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        // Unique temp roots keep preset tests isolated from each other
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "unixnotis-preset-export-{}-{}-{}",
            name, stamp, serial
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) {
        // Helper keeps test setup focused on intent instead of path plumbing
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
fn export_builds_bundle_from_config_root() {
    // Export should pack the live config tree into one bundle file
    let root = TempDirGuard::new("bundle");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    root.write("base.css", ".panel { color: red; }");
    root.write("assets/bg.png", "png");

    let bundle_path = root.path.join("demo.unixnotis");
    let summary = export_preset_from(&root.path, &bundle_path, &[], false).expect("export");

    assert_eq!(summary.file_count, 3);
    assert!(bundle_path.exists());
}

#[test]
fn export_rejects_excluding_config_toml() {
    // config.toml is required for a usable preset
    let root = TempDirGuard::new("exclude-config");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    root.write("base.css", ".panel { color: red; }");

    let bundle_path = root.path.join("demo.unixnotis");
    let error = export_preset_from(
        &root.path,
        &bundle_path,
        &["config.toml".to_string()],
        false,
    )
    .expect_err("reject config exclusion");

    assert!(error.to_string().contains("cannot exclude config.toml"));
}

#[test]
fn export_rejects_theme_paths_that_leave_root_through_parent_traversal() {
    // Lexical `../` escapes should be blocked during export, not discovered later on import
    let root = TempDirGuard::new("parent-theme-escape");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"../outside.css\"\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n",
    );
    root.write("panel.css", ".panel { color: red; }");
    root.write("popup.css", ".popup { color: red; }");
    root.write("widgets.css", ".widgets { color: red; }");
    root.write("media.css", ".media { color: red; }");

    let bundle_path = root.path.join("demo.unixnotis");
    let error =
        export_preset_from(&root.path, &bundle_path, &[], false).expect_err("reject theme escape");

    assert!(error
        .to_string()
        .contains("requires base_css to live under the config root"));
    assert!(!bundle_path.exists());
}

#[test]
fn export_rejects_absolute_plugin_command_outside_root() {
    // Shared presets should stay self-contained when commands point at local scripts
    let root = TempDirGuard::new("outside-command");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\n[widgets.stats.plugin]\napi_version = 1\ncommand = \"/tmp/outside-plugin\"\n",
    );
    root.write("base.css", ".panel { color: red; }");

    let bundle_path = root.path.join("demo.unixnotis");
    let error = export_preset_from(&root.path, &bundle_path, &[], false)
        .expect_err("reject outside plugin command");

    assert!(error
        .to_string()
        .contains("explicit command paths to stay under the config root"));
    assert!(!bundle_path.exists());
}

#[cfg(unix)]
#[test]
fn export_replaces_symlink_output_instead_of_following_it() {
    // Export should replace the symlink path itself, not overwrite the symlink target
    let root = TempDirGuard::new("symlink-output");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    root.write("base.css", ".panel { color: red; }");

    let outside_target = root.path.with_file_name("outside-target.unixnotis");
    fs::write(&outside_target, "ORIGINAL").expect("write outside target");
    let bundle_path = root.path.join("bundle.unixnotis");
    std::os::unix::fs::symlink(&outside_target, &bundle_path).expect("create output symlink");

    let summary = export_preset_from(&root.path, &bundle_path, &[], true).expect("export");

    assert_eq!(summary.file_count, 2);
    assert_eq!(
        fs::read_to_string(&outside_target).expect("read outside target"),
        "ORIGINAL"
    );
    assert!(bundle_path.exists());
    assert!(!fs::symlink_metadata(&bundle_path)
        .expect("stat bundle path")
        .file_type()
        .is_symlink());
}

#[test]
fn export_rejects_outside_css_asset_refs_in_noninteractive_runs() {
    // Shared presets should pause when CSS reaches outside the config root for assets
    let root = TempDirGuard::new("external-css-asset");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    root.write(
        "base.css",
        ".panel { background-image: url(\"../outside.png\"); }\n",
    );

    let bundle_path = root.path.join("demo.unixnotis");
    // The injected rejector makes this test stable even when cargo test has a tty
    let error = export_preset_from_with_confirm(
        &root.path,
        &bundle_path,
        &[],
        false,
        |_refs| {
            Err(anyhow!(
                "preset export found CSS asset references outside the UnixNotis config directory"
            ))
        },
        |_leaks| Ok(false),
    )
    .expect_err("reject outside css asset refs without confirmation");

    assert!(error
        .to_string()
        .contains("CSS asset references outside the UnixNotis config directory"));
    assert!(!bundle_path.exists());
}

#[test]
fn export_can_rewrite_host_specific_command_paths_in_bundle_config() {
    // Shared presets can fix leaked home paths in the bundled config without touching the live tree
    let root = TempDirGuard::new("host-specific-command-path");
    let script_path = root.path.join("scripts/unixnotis-thermal-stat");
    root.write(
        "config.toml",
        &format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\n[widgets.stats.plugin]\napi_version = 1\ncommand = {:?}\n",
            script_path.display().to_string()
        ),
    );
    root.write("base.css", ".panel { color: red; }");
    root.write("scripts/unixnotis-thermal-stat", "#!/bin/sh\necho 42\n");

    let bundle_path = root.path.join("demo.unixnotis");
    let summary = export_preset_from_with_confirm(
        &root.path,
        &bundle_path,
        &[],
        false,
        |_refs| Ok(()),
        |_leaks| Ok(true),
    )
    .expect("export with rewrite");

    assert_eq!(summary.file_count, 3);
    let bundle = read_bundle(&bundle_path).expect("read bundle");
    let config_file = bundle
        .files
        .iter()
        .find(|file| file.relative_path == Path::new("config.toml"))
        .expect("bundled config");
    let config_text = std::str::from_utf8(&config_file.contents).expect("utf8 config");

    assert!(config_text.contains("scripts/unixnotis-thermal-stat"));
    assert!(!config_text.contains(&script_path.display().to_string()));
    let live_config = fs::read_to_string(root.path.join("config.toml")).expect("live config");
    assert!(live_config.contains(&script_path.display().to_string()));
}

#[test]
fn export_can_keep_host_specific_command_paths_when_fix_is_declined() {
    // Declining the helper should keep the bundle exportable without mutating the command text
    let root = TempDirGuard::new("keep-host-specific-command-path");
    let script_path = root.path.join("scripts/unixnotis-thermal-stat");
    root.write(
        "config.toml",
        &format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\n[widgets.stats.plugin]\napi_version = 1\ncommand = {:?}\n",
            script_path.display().to_string()
        ),
    );
    root.write("base.css", ".panel { color: red; }");
    root.write("scripts/unixnotis-thermal-stat", "#!/bin/sh\necho 42\n");

    let bundle_path = root.path.join("demo.unixnotis");
    export_preset_from_with_confirm(
        &root.path,
        &bundle_path,
        &[],
        false,
        |_refs| Ok(()),
        |_leaks| Ok(false),
    )
    .expect("export without rewrite");

    let bundle = read_bundle(&bundle_path).expect("read bundle");
    let config_file = bundle
        .files
        .iter()
        .find(|file| file.relative_path == Path::new("config.toml"))
        .expect("bundled config");
    let config_text = std::str::from_utf8(&config_file.contents).expect("utf8 config");

    assert!(config_text.contains(&script_path.display().to_string()));
}
