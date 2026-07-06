use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::config_root::{collect_config_files, override_collected_file_contents};
use super::super::pathing::format_relative_path;

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
        let path = std::env::temp_dir().join(format!(
            "unixnotis-preset-filesystem-{}-{}-{}",
            name, stamp, serial
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) {
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
fn collect_config_files_skips_backups_symlinks_and_output_file() {
    // Export should keep the tree portable and avoid self-inclusion
    let root = TempDirGuard::new("collect");
    root.write("config.toml", "demo = true");
    root.write("assets/bg.png", "png");
    root.write("Backup-2026-04-11/config.toml", "old");
    root.write("scripts/run.sh", "echo hi");
    root.write("bundle.unixnotis", "old bundle");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.path.join("assets/bg.png"),
        root.path.join("linked.png"),
    )
    .expect("create symlink");

    let collected = collect_config_files(
        &root.path,
        Some(&root.path.join("bundle.unixnotis")),
        &[PathBuf::from("scripts")],
    )
    .expect("collect files");

    let paths = collected
        .files
        .iter()
        .map(|file| format_relative_path(&file.relative_path))
        .collect::<Vec<_>>();
    assert_eq!(paths, vec!["assets/bg.png", "config.toml"]);
    #[cfg(unix)]
    assert_eq!(
        collected
            .skipped_symlinks
            .iter()
            .map(|path| format_relative_path(path))
            .collect::<Vec<_>>(),
        vec!["linked.png"]
    );
}

#[cfg(unix)]
#[test]
fn collect_config_files_rejects_special_permission_bits() {
    let root = TempDirGuard::new("special-mode");
    root.write("config.toml", "demo = true");
    root.write("scripts/run.sh", "#!/bin/sh\necho hi\n");

    let script_path = root.path.join("scripts/run.sh");
    let mut perms = fs::metadata(&script_path)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o4755);
    fs::set_permissions(&script_path, perms).expect("set script mode");

    let error = collect_config_files(&root.path, None, &[]).expect_err("reject special mode");
    assert!(error.to_string().contains("special permission bits"));
}

#[test]
fn override_collected_file_contents_updates_manifest_size() {
    let root = TempDirGuard::new("override");
    root.write("config.toml", "demo = true");

    let mut collected = collect_config_files(&root.path, None, &[]).expect("collect files");
    override_collected_file_contents(
        &mut collected,
        Path::new("config.toml"),
        b"demo = false\n".to_vec(),
    )
    .expect("override config");

    let config = collected
        .files
        .iter()
        .find(|file| file.relative_path == Path::new("config.toml"))
        .expect("config file");
    assert_eq!(config.size, b"demo = false\n".len() as u64);
    assert_eq!(
        config.contents_override.as_deref(),
        Some(&b"demo = false\n"[..])
    );
}
