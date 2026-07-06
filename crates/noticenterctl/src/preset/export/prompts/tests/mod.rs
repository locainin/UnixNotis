use super::{
    rewrite_host_specific_css_asset_refs_if_requested,
    rewrite_host_specific_script_paths_if_requested,
};
use crate::preset::config_root::{CollectedConfigFiles, PresetFileSource};
use anyhow::anyhow;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("unixnotis-export-prompts-{name}-{stamp}-{serial}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &[u8]) -> PresetFileSource {
        let source_path = self.path.join(relative_path);
        if let Some(parent) = source_path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&source_path, contents).expect("write file");
        let metadata = fs::metadata(&source_path).expect("metadata");
        #[cfg(unix)]
        let mode = metadata.permissions().mode() & 0o777;
        #[cfg(not(unix))]
        let mode = 0o644;

        PresetFileSource {
            relative_path: PathBuf::from(relative_path),
            source_path,
            size: metadata.len(),
            mode,
            contents_override: None,
        }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn css_prompt_error_restores_staged_bytes_and_size() {
    let root = TempDirGuard::new("css-restore");
    let asset_path = root.path.join("assets/example.png");
    fs::create_dir_all(asset_path.parent().expect("asset parent")).expect("create asset dir");
    fs::write(&asset_path, b"png").expect("write asset");
    let original = format!(
        ".panel {{ background-image: url(\"file://{}\"); }}\n",
        asset_path.display()
    );
    let file = root.write("base.css", original.as_bytes());
    let original_size = file.size;
    let mut collected = CollectedConfigFiles {
        files: vec![file],
        ..CollectedConfigFiles::default()
    };

    let error =
        rewrite_host_specific_css_asset_refs_if_requested(&root.path, &mut collected, |_leaks| {
            Err(anyhow!("prompt failed"))
        })
        .expect_err("prompt should fail");

    assert!(error.to_string().contains("prompt failed"));
    assert!(collected.files[0].contents_override.is_none());
    assert_eq!(collected.files[0].size, original_size);
}

#[test]
fn script_prompt_error_restores_staged_bytes_and_size() {
    let root = TempDirGuard::new("script-restore");
    let original = format!(
        "#!/bin/sh\necho \"{}/assets/example.png\"\n",
        root.path.display()
    );
    let file = root.write("scripts/demo-widget", original.as_bytes());
    let original_size = file.size;
    let mut collected = CollectedConfigFiles {
        files: vec![file],
        ..CollectedConfigFiles::default()
    };

    let error =
        rewrite_host_specific_script_paths_if_requested(&root.path, &mut collected, |_leaks| {
            Err(anyhow!("prompt failed"))
        })
        .expect_err("prompt should fail");

    assert!(error.to_string().contains("prompt failed"));
    assert!(collected.files[0].contents_override.is_none());
    assert_eq!(collected.files[0].size, original_size);
}
