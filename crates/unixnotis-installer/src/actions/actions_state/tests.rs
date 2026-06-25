use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

use super::check_install_state;

#[test]
fn dinit_artifact_backed_enablement_does_not_log_missing_enabled_command_error() {
    let root = test_root("dinit-artifact-enabled-state");
    let service_root = root.join("dinit.d");
    let boot_dir = service_root.join("boot.d");
    fs::create_dir_all(&boot_dir).expect("boot dir");
    fs::write(
        service_root.join("unixnotis-daemon"),
        "type = process\ncommand = /tmp/bin/unixnotis-daemon\n",
    )
    .expect("service file");
    symlink("../unixnotis-daemon", boot_dir.join("unixnotis-daemon")).expect("boot symlink");

    let paths = InstallPaths {
        repo_root: repo_root(),
        bin_dir: root.join("bin"),
        service: ServiceManager::dinit_user(service_root),
    };

    let state = check_install_state(&paths);

    assert!(state.service_enabled);
    assert!(state.service_enabled_error.is_none());

    let _ = fs::remove_dir_all(root);
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
