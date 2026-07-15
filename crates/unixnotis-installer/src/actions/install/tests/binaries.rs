use std::fs;

use crate::detect::Detection;
use crate::model::ActionMode;

use super::super::binaries::{
    binary_temp_path, binary_temp_path_attempt, stage_binary_copy_with_retry,
};
use super::super::{install_binaries, remove_binaries};
use super::support::{test_context, test_paths, test_root, write_fake_workspace};

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[test]
fn install_binaries_copies_all_managed_binaries_including_noticenterctl() {
    let _lock = crate::test_support::env::test_env_lock();
    // A fake workspace keeps the test focused on copy behavior instead of the real repo layout
    let root = test_root("install-binaries");
    write_fake_workspace(
        &root,
        &[
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ],
    );
    let paths = test_paths(&root);

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        let source = paths.repo_root.join("target").join("release").join(binary);
        fs::create_dir_all(source.parent().expect("release dir")).expect("make release dir");
        fs::write(&source, format!("binary:{binary}")).expect("write fake binary");
    }

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);

    install_binaries(&mut ctx).expect("install should copy binaries");

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        let installed = paths.bin_dir.join(binary);
        assert!(installed.exists(), "{binary} should be installed");
        assert_eq!(
            fs::read_to_string(&installed).expect("read installed binary"),
            format!("binary:{binary}")
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_binaries_copies_from_release_archive_bin_dir() {
    let root = test_root("install-release-archive-binaries");
    let paths = test_paths(&root);
    write_fake_release_archive(&root);

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);

    install_binaries(&mut ctx).expect("release archive install should copy binaries");

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        let installed = paths.bin_dir.join(binary);
        assert!(installed.exists(), "{binary} should be installed");
        assert_eq!(
            fs::read_to_string(&installed).expect("read installed binary"),
            format!("release:{binary}")
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn install_binaries_bypasses_preexisting_temp_symlink_without_touching_it() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("install-binaries-temp-symlink");
    write_fake_workspace(
        &root,
        &[
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ],
    );
    let paths = test_paths(&root);
    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        let source = paths.repo_root.join("target").join("release").join(binary);
        fs::create_dir_all(source.parent().expect("release dir")).expect("make release dir");
        fs::write(&source, format!("binary:{binary}")).expect("write fake binary");
    }
    fs::create_dir_all(&paths.bin_dir).expect("bin dir");
    let destination = paths.bin_dir.join("unixnotis-daemon");
    let temp_path = binary_temp_path(&destination);
    let protected = root.join("protected");
    fs::write(&protected, "protected").expect("protected");
    symlink(&protected, &temp_path).expect("temp symlink");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);

    install_binaries(&mut ctx).expect("alternate temp path should bypass stale symlink");

    assert_eq!(
        fs::read_to_string(&protected).expect("protected remains"),
        "protected"
    );
    assert!(fs::symlink_metadata(&temp_path)
        .expect("temp symlink remains")
        .file_type()
        .is_symlink());
    assert!(destination.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_temp_path_attempt_uses_stable_first_path_and_unique_retry_path() {
    let destination = std::env::temp_dir().join("unixnotis-daemon");

    let first = binary_temp_path_attempt(&destination, 0);
    let retry = binary_temp_path_attempt(&destination, 1);

    // The stable first path makes stale-file handling deterministic and testable
    assert_eq!(first, binary_temp_path(&destination));
    // Retry paths carry the attempt so a collision cannot repeat the first candidate
    assert_ne!(retry, first);
    assert!(retry
        .file_name()
        .expect("retry file name")
        .to_string_lossy()
        .ends_with("-1"));
}

#[test]
fn stage_binary_copy_propagates_errors_other_than_path_collisions() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-installer-binary-stage-error-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let source_dir = root.join("source-directory");
    let destination = root.join("unixnotis-daemon");
    fs::create_dir_all(&source_dir).expect("make invalid directory source");

    let error = stage_binary_copy_with_retry(&source_dir, &destination)
        .expect_err("a source read error must not be treated as a temp collision");

    assert_ne!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(!destination.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_binaries_removes_all_managed_binaries_including_noticenterctl() {
    // Uninstall must remove the same managed set that install copied in
    let root = test_root("remove-binaries");
    write_fake_workspace(
        &root,
        &[
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ],
    );
    let paths = test_paths(&root);

    fs::create_dir_all(&paths.bin_dir).expect("make bin dir");
    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        fs::write(paths.bin_dir.join(binary), format!("installed:{binary}"))
            .expect("write installed binary");
    }

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);

    remove_binaries(&mut ctx).expect("remove should delete binaries");

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        assert!(
            !paths.bin_dir.join(binary).exists(),
            "{binary} should be removed"
        );
    }

    let _ = fs::remove_dir_all(&root);
}

fn write_fake_release_archive(root: &std::path::Path) {
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("release bin dir");
    fs::write(
        root.join("unixnotis-release.json"),
        r#"{"version":"1.0.0","binaries":["unixnotis-daemon","unixnotis-popups","unixnotis-center","noticenterctl"]}"#,
    )
    .expect("release manifest");

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        fs::write(bin_dir.join(binary), format!("release:{binary}")).expect("release binary");
    }
}
