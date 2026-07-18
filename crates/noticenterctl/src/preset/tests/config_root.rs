use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::config_root::{
    checked_export_total, collect_selected_config_files_from_root,
    override_collected_file_contents, CollectedConfigFiles, SecureFileCapture,
};
use super::super::filesystem::open_secure_dir_all;
use super::super::pathing::format_relative_path;
use std::collections::BTreeMap;

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn collect_selected_config_files(
    config_dir: &Path,
    relative_paths: &[PathBuf],
    output_path: Option<&Path>,
    exclusions: &[PathBuf],
) -> anyhow::Result<CollectedConfigFiles> {
    collect_selected_config_files_with_captures(
        config_dir,
        relative_paths,
        output_path,
        exclusions,
        &BTreeMap::new(),
    )
}

fn collect_selected_config_files_with_captures(
    config_dir: &Path,
    relative_paths: &[PathBuf],
    output_path: Option<&Path>,
    exclusions: &[PathBuf],
    captures: &BTreeMap<PathBuf, SecureFileCapture>,
) -> anyhow::Result<CollectedConfigFiles> {
    let root_fd = open_secure_dir_all(config_dir)?;
    collect_selected_config_files_from_root(
        &root_fd,
        config_dir,
        relative_paths,
        output_path,
        exclusions,
        captures,
    )
}

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
            "unixnotis-preset-filesystem-{name}-{stamp}-{serial}"
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
fn selected_collection_keeps_only_dependencies_and_skips_unsafe_entries() {
    // Export should keep the dependency set portable and avoid unrelated content
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

    let collected = collect_selected_config_files(
        &root.path,
        &[
            PathBuf::from("config.toml"),
            PathBuf::from("assets/bg.png"),
            PathBuf::from("linked.png"),
            PathBuf::from("bundle.unixnotis"),
        ],
        Some(&root.path.join("bundle.unixnotis")),
        &[],
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
fn selected_collection_rejects_special_permission_bits() {
    let root = TempDirGuard::new("special-mode");
    root.write("config.toml", "demo = true");
    root.write("scripts/run.sh", "#!/bin/sh\necho hi\n");

    let script_path = root.path.join("scripts/run.sh");
    let mut perms = fs::metadata(&script_path)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o4755);
    fs::set_permissions(&script_path, perms).expect("set script mode");

    let error =
        collect_selected_config_files(&root.path, &[PathBuf::from("scripts/run.sh")], None, &[])
            .expect_err("reject special mode");
    assert!(error.to_string().contains("special permission bits"));
}

#[test]
fn override_collected_file_contents_updates_manifest_size() {
    let root = TempDirGuard::new("override");
    root.write("config.toml", "demo = true");

    let mut collected =
        collect_selected_config_files(&root.path, &[PathBuf::from("config.toml")], None, &[])
            .expect("collect files");
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

#[test]
fn selected_collection_keeps_descriptor_validated_bytes_after_path_replacement() {
    let root = TempDirGuard::new("captured-bytes");
    root.write("config.toml", "trusted = true");
    let collected =
        collect_selected_config_files(&root.path, &[PathBuf::from("config.toml")], None, &[])
            .expect("collect file");

    fs::write(root.path.join("config.toml"), "replaced = true").expect("replace live path");

    assert_eq!(collected.files[0].source_contents, b"trusted = true");
    assert_eq!(collected.files[0].size, b"trusted = true".len() as u64);
}

#[test]
fn export_totals_accept_exact_limits_and_reject_overflow() {
    assert_eq!(
        checked_export_total(67_108_863, 1, 511).expect("exact limit"),
        67_108_864
    );
    assert!(checked_export_total(67_108_864, 1, 0).is_err());
    assert!(checked_export_total(u64::MAX, 1, 0).is_err());
    assert!(checked_export_total(0, 0, 512).is_err());
}
