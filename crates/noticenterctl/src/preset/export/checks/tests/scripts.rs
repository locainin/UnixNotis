use super::{
    capture_file_overrides, restore_file_overrides, rewrite_host_specific_script_paths_in_sources,
};
use crate::preset::config_root::PresetFileSource;
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
            std::env::temp_dir().join(format!("unixnotis-script-rewrite-{name}-{stamp}-{serial}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write_bytes(&self, relative_path: &str, contents: &[u8]) -> PathBuf {
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

fn make_source(root: &TempDirGuard, relative_path: &str, contents: &[u8]) -> PresetFileSource {
    let source_path = root.write_bytes(relative_path, contents);
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
        source_contents: contents.to_vec(),
        contents_override: None,
    }
}

#[test]
fn script_rewrite_skips_non_utf8_files_instead_of_corrupting_them() {
    let root = TempDirGuard::new("binary");
    let binary = make_source(
        &root,
        "scripts/demo-widget",
        &[0xff, 0xfe, 0xfd, 0x00, b'/', b'h', b'o', b'm', b'e'],
    );
    let original_size = binary.size;
    let mut files = vec![binary];

    let leaks = rewrite_host_specific_script_paths_in_sources(&root.path, &mut files)
        .expect("rewrite check");

    assert!(leaks.is_empty());
    assert!(files[0].contents_override.is_none());
    assert_eq!(files[0].size, original_size);
}

#[test]
fn script_rewrite_reports_all_matched_needles_for_one_file() {
    let root = TempDirGuard::new("multi-needle");
    let root_text = root.path.display().to_string();
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"{root_text}/assets/a.png\"\nprintf '%s\\n' \"file://{root_text}/assets/b.png\"\n"
    );
    let mut files = vec![make_source(&root, "scripts/demo-widget", script.as_bytes())];

    let leaks = rewrite_host_specific_script_paths_in_sources(&root.path, &mut files)
        .expect("rewrite check");

    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].needles.len(), 2);
    assert!(files[0]
        .contents_override
        .as_ref()
        .expect("override bytes")
        .windows(b"${XDG_CONFIG_HOME:-$HOME/.config}/unixnotis".len())
        .any(|window| window == b"${XDG_CONFIG_HOME:-$HOME/.config}/unixnotis"));
}

#[test]
fn restore_file_overrides_recovers_size_and_bytes_without_metadata() {
    let root = TempDirGuard::new("restore");
    let mut files = vec![make_source(
        &root,
        "scripts/demo-widget",
        b"#!/bin/sh\necho ok\n",
    )];
    let snapshots = capture_file_overrides(&files);

    files[0].size = 999;
    files[0].contents_override = Some(b"changed".to_vec());
    fs::remove_file(&files[0].source_path).expect("remove source file");

    restore_file_overrides(&mut files, &snapshots);

    assert_eq!(files[0].size, snapshots[0].size);
    assert_eq!(files[0].contents_override, snapshots[0].contents_override);
}
