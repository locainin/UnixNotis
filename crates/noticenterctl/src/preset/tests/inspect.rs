use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Builder, Header};

use super::super::export::flow::export_preset_from;
use super::super::inspect::inspect_preset_at;
use super::super::manifest::{PresetManifest, PresetManifestFile};

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
        let path =
            std::env::temp_dir().join(format!("unixnotis-preset-inspect-{name}-{stamp}-{serial}"));
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
    assert!(report.contains("widgets requested:"));
    assert!(report.contains("widgets.volume.get_cmd"));
    assert!(report.contains("command path warnings:"));
    assert!(report.contains("host-specific command paths:"));
    assert!(report.contains("file list:"));
    assert!(report.contains("config.toml"));
}

#[test]
fn inspect_warns_before_an_oversized_widget_tree_is_imported() {
    let root = TempDirGuard::new("widget-limits");
    let bundle_path = root.path.join("widget-limits.unixnotis");
    let mut config = String::from("[theme]\nbase_css = \"base.css\"\n");
    for index in 0..20 {
        config.push_str(&format!(
            "[[widgets.toggles]]\nlabel = \"Toggle {index}\"\nicon = \"network-wireless-symbolic\"\n"
        ));
    }
    write_bundle_with_files(&bundle_path, "widget-limits", &[("config.toml", &config)]);

    let report = inspect_preset_at(&bundle_path).expect("inspect");

    assert!(report.contains("widgets requested:"));
    assert!(report.contains("toggles=20"));
    assert!(report.contains("widget limit warning:"));
}

#[test]
fn inspect_sanitizes_preset_control_sequences_before_terminal_output() {
    // A shared preset can name itself and its commands, but it must not control the terminal
    let root = TempDirGuard::new("terminal-controls");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[widgets.volume]\nget_cmd = \"printf '\\u001b]52;c;AAAA\\u0007'\"\n",
    );
    root.write("base.css", ".a { color: red; }");
    let bundle_path = root.path.join("demo\u{1b}]0;owned\u{7}.unixnotis");
    export_preset_from(&root.path, &bundle_path, &[], true).expect("export");

    let report = inspect_preset_at(&bundle_path).expect("inspect");

    assert!(!report.contains('\u{1b}'));
    assert!(!report.contains('\u{7}'));
    assert!(report.contains("preset: demo ]0;owned"));
    assert!(
        report.contains("printf  ]52;c;AAAA"),
        "sanitized command missing from report: {report:?}"
    );
}

#[test]
fn inspect_reports_assets_scripts_and_css_warnings() {
    // Summary flags and CSS warnings should stay visible because they drive import review
    let root = TempDirGuard::new("flags-css");
    let bundle_path = root.path.join("demo.unixnotis");
    write_bundle_with_files(
        &bundle_path,
        "demo",
        &[
            ("config.toml", "[theme]\nbase_css = \"base.css\"\n"),
            (
                "base.css",
                ".a { background-image: url(\"https://example.com/a.png\"); }",
            ),
            (
                "assets/icon.svg",
                "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            ),
            ("scripts/status.sh", "#!/bin/sh\ntrue\n"),
        ],
    );

    let report = inspect_preset_at(&bundle_path).expect("inspect");

    assert!(report.contains("assets: yes"));
    assert!(report.contains("scripts: yes"));
    assert!(report.contains("css asset path warnings: 1"));
    assert!(report.contains("$XDG_CONFIG_HOME/unixnotis/base.css -> https://example.com/a.png"));
}

#[test]
fn inspect_reports_theme_paths_that_leave_config_root() {
    // Inspect must warn on hostile bundles even when export would refuse to create them
    let root = TempDirGuard::new("theme-warning");
    let bundle_path = root.path.join("theme-warning.unixnotis");
    write_bundle_with_files(
        &bundle_path,
        "theme-warning",
        &[(
            "config.toml",
            "[theme]\nbase_css = \"/tmp/evil.css\"\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n",
        )],
    );

    let report = inspect_preset_at(&bundle_path).expect("inspect");

    assert!(report.contains("theme path warnings: 1"));
    assert!(report.contains("theme.base_css points outside the config root: /tmp/evil.css"));
}

fn write_bundle_with_files(bundle_path: &PathBuf, bundle_name: &str, files: &[(&str, &str)]) {
    // Raw bundle writing lets inspect tests model presets that export would reject
    let output = fs::File::create(bundle_path).expect("create test bundle");
    let encoder = GzEncoder::new(output, Compression::default());
    let mut archive = Builder::new(encoder);
    let manifest = PresetManifest::new(
        bundle_name.to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        "test".to_string(),
        files
            .iter()
            .map(|(path, contents)| PresetManifestFile {
                path: (*path).to_string(),
                size: contents.len() as u64,
            })
            .collect(),
    );
    let manifest_text = manifest.encode().expect("encode manifest");

    append_text_file(&mut archive, "manifest.toml", &manifest_text);
    for (path, contents) in files {
        append_text_file(&mut archive, &format!("payload/{path}"), contents);
    }
    archive.finish().expect("finish archive");
}

fn append_text_file<W: Write>(archive: &mut Builder<W>, path: &str, contents: &str) {
    // Minimal tar entries are enough for read_bundle to exercise its normal validation
    let mut header = Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, path, contents.as_bytes())
        .expect("append text file");
}
