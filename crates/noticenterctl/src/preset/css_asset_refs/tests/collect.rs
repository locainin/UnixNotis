use super::super::{
    collect_external_css_asset_refs_from_bundle, collect_external_css_asset_refs_from_collected,
    collect_external_css_asset_refs_from_paths, collect_local_css_asset_paths_from_captures,
};
use crate::preset::archive::BundleFile;
use crate::preset::config_root::{PresetFileSource, SecureFileCapture};
use std::collections::BTreeMap;
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
        let path =
            std::env::temp_dir().join(format!("unixnotis-css-asset-refs-{name}-{stamp}-{serial}"));
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
    )
    .expect("scan bundle CSS");

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

#[test]
fn finds_remote_url_in_live_css() {
    // Remote URLs stay valid css syntax, but they are still called out as non-local asset refs
    let root = TempDirGuard::new("remote");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let css_path = root.write(
        "xdg/unixnotis/base.css",
        ".panel { background-image: url(\"https://example.com/panel.png\"); }\n",
    );

    let refs =
        collect_external_css_asset_refs_from_paths(&config_dir, &[css_path]).expect("scan css");

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reason, "remote url");
}

#[test]
fn local_dependency_collection_ignores_embedded_and_remote_urls() {
    let root = TempDirGuard::new("local-dependencies");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let css_path = root.write(
        "xdg/unixnotis/base.css",
        ".a { background: url('assets/local.png'); }\n\
         .b { background: url('data:image/png;base64,AAAA'); }\n\
         .c { background: url('http://example.com/a.png'); }\n\
         .d { background: url('https://example.com/b.png'); }\n",
    );
    let captures = BTreeMap::from([(
        PathBuf::from("base.css"),
        SecureFileCapture {
            contents: fs::read(&css_path).expect("read css fixture"),
            mode: 0o644,
        },
    )]);

    let paths = collect_local_css_asset_paths_from_captures(&config_dir, &[css_path], &captures)
        .expect("collect local dependencies");

    assert_eq!(paths, vec![PathBuf::from("assets/local.png")]);
}

#[test]
fn local_dependency_collection_uses_captured_stylesheet_bytes() {
    let root = TempDirGuard::new("captured-local-dependencies");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let css_path = root.write(
        "xdg/unixnotis/base.css",
        ".panel { background: url('assets/replaced.png'); }\n",
    );
    let captures = BTreeMap::from([(
        PathBuf::from("base.css"),
        SecureFileCapture {
            contents: b".panel { background: url('assets/captured.png'); }\n".to_vec(),
            mode: 0o644,
        },
    )]);

    let paths = collect_local_css_asset_paths_from_captures(&config_dir, &[css_path], &captures)
        .expect("collect captured local dependencies");

    assert_eq!(paths, vec![PathBuf::from("assets/captured.png")]);
}

#[test]
fn quoted_import_outside_root_requires_review() {
    let root = TempDirGuard::new("quoted-import");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let css_path = root.write("xdg/unixnotis/base.css", "@import \"../outside.css\";\n");

    let refs =
        collect_external_css_asset_refs_from_paths(&config_dir, &[css_path]).expect("scan css");

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].asset_ref, "../outside.css");
    assert_eq!(refs[0].reason, "relative path leaves the config root");
}

#[test]
fn quoted_relative_import_is_selected_for_export() {
    let root = TempDirGuard::new("quoted-import-local");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let css_path = root.write(
        "xdg/unixnotis/styles/base.css",
        "@import \"../shared/colors.css\";\n",
    );
    let captures = BTreeMap::from([(
        PathBuf::from("styles/base.css"),
        SecureFileCapture {
            contents: fs::read(&css_path).expect("read css fixture"),
            mode: 0o644,
        },
    )]);

    let paths = collect_local_css_asset_paths_from_captures(&config_dir, &[css_path], &captures)
        .expect("collect local dependencies");

    assert_eq!(paths, vec![PathBuf::from("shared/colors.css")]);
}

#[test]
fn escaped_import_and_url_require_review() {
    let root = TempDirGuard::new("escaped-refs");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let css_path = root.write(
        "xdg/unixnotis/base.css",
        "@import \"\\2f tmp/outside.css\";\n.a { background: url(\"\\2f tmp/a.png\"); }\n",
    );

    let refs =
        collect_external_css_asset_refs_from_paths(&config_dir, &[css_path]).expect("scan css");

    assert_eq!(refs.len(), 2);
    assert!(refs
        .iter()
        .any(|finding| finding.reason == "unrecognized CSS import syntax"));
    assert!(refs
        .iter()
        .any(|finding| finding.reason == "unrecognized CSS url syntax"));
}

#[test]
fn collected_export_css_uses_captured_source_bytes() {
    let root = TempDirGuard::new("collected-source");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let source_path = root.write(
        "xdg/unixnotis/base.css",
        ".panel { background: url('assets/live.png'); }",
    );
    let source_contents =
        b".panel { background: url('https://example.invalid/captured.png'); }".to_vec();
    let files = [PresetFileSource {
        relative_path: PathBuf::from("base.css"),
        source_path,
        size: u64::try_from(source_contents.len()).expect("fixture length"),
        mode: 0o644,
        source_contents,
        contents_override: None,
    }];

    let refs = collect_external_css_asset_refs_from_collected(&config_dir, &files)
        .expect("scan collected CSS");

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reason, "remote url");
}

#[test]
fn bundle_wide_external_reference_limit_is_exact_and_bounded() {
    let root = TempDirGuard::new("bundle-reference-limit");
    let config_dir = root.path.join("xdg/unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let first = "url(https://example.invalid/a)".repeat(2_048);
    let exact = "url(https://example.invalid/b)".repeat(2_048);
    let over = "url(https://example.invalid/b)".repeat(2_049);

    let exact_refs = collect_external_css_asset_refs_from_bundle(
        &config_dir,
        &[
            BundleFile {
                relative_path: PathBuf::from("first.css"),
                contents: first.as_bytes().to_vec(),
                mode: 0o644,
            },
            BundleFile {
                relative_path: PathBuf::from("exact.css"),
                contents: exact.into_bytes(),
                mode: 0o644,
            },
        ],
    )
    .expect("exact bundle-wide reference limit");
    assert_eq!(exact_refs.len(), 4_096);

    let error = collect_external_css_asset_refs_from_bundle(
        &config_dir,
        &[
            BundleFile {
                relative_path: PathBuf::from("first.css"),
                contents: first.into_bytes(),
                mode: 0o644,
            },
            BundleFile {
                relative_path: PathBuf::from("over.css"),
                contents: over.into_bytes(),
                mode: 0o644,
            },
        ],
    )
    .expect_err("bundle-wide reference limit must reject the first excess item");

    assert!(error
        .to_string()
        .contains("more than 4096 external references"));
}
