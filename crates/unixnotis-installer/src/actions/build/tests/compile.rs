use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

use super::{add_binary_targets, run_build};

#[test]
fn source_build_selects_binary_targets_instead_of_assuming_package_names() {
    let mut command = std::process::Command::new("cargo");
    command.args(["build", "--release"]);

    add_binary_targets(
        &mut command,
        &["runtime-package".to_string(), "runtime-helper".to_string()],
    );

    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        args,
        [
            "build",
            "--release",
            "--bin",
            "runtime-package",
            "--bin",
            "runtime-helper"
        ]
    );
    assert!(!args.iter().any(|arg| arg == "-p"));
}

#[test]
fn run_build_accepts_complete_release_archive_without_cargo() {
    let root = test_root("release-build-complete");
    write_fake_release_archive(&root, true);
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths);

    run_build(&mut ctx).expect("complete release archive should prepare binaries");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn run_build_rejects_release_archive_with_missing_bundled_binary() {
    let root = test_root("release-build-missing");
    write_fake_release_archive(&root, false);
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths);

    let err = run_build(&mut ctx).expect_err("missing binary should fail");

    // Archive installs must stop before copying files when the bundle is incomplete
    assert!(err.to_string().contains("missing bundled release binaries"));

    let _ = fs::remove_dir_all(root);
}

fn test_context<'a>(_detection: &'a Detection, paths: &'a InstallPaths) -> ActionContext<'a> {
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(32);
    ActionContext {
        paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
}

fn test_paths(root: &std::path::Path) -> InstallPaths {
    InstallPaths {
        repo_root: root.to_path_buf(),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: ServiceManager::systemd_user(root.join("home").join(".config/systemd/user")),
    }
}

fn test_root(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("unixnotis-installer-{name}-{stamp}"))
}

fn write_fake_release_archive(root: &std::path::Path, complete: bool) {
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("release bin dir");
    fs::write(
        root.join("unixnotis-release.json"),
        r#"{"version":"1.0.0","binaries":["unixnotis-daemon","unixnotis-popups","unixnotis-center","noticenterctl"]}"#,
    )
    .expect("release manifest");

    let binaries = if complete {
        [
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ]
        .as_slice()
    } else {
        ["unixnotis-daemon", "unixnotis-popups", "unixnotis-center"].as_slice()
    };

    for binary in binaries {
        fs::write(bin_dir.join(binary), format!("release:{binary}")).expect("release binary");
    }
}
