use std::fs;

use crate::detect::Detection;
use crate::model::ActionMode;

use super::super::binaries::remove_resolved_binaries;
use super::super::{install_binaries, remove_binaries};
use super::support::{test_context, test_paths, test_root, write_fake_workspace};

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};

#[test]
fn install_binaries_copies_all_managed_binaries_and_runtime_helpers() {
    let _lock = crate::test_support::env::test_env_lock();
    // A fake workspace keeps the test focused on copy behavior instead of the real repo layout
    let root = test_root("install-binaries");
    write_fake_workspace(
        &root,
        &[
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "unixnotis-css-validate",
            "noticenterctl",
        ],
    );
    let paths = test_paths(&root);

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "unixnotis-css-validate",
        "noticenterctl",
    ] {
        let source = paths.repo_root.join("target").join("release").join(binary);
        fs::create_dir_all(source.parent().expect("release dir")).expect("make release dir");
        fs::write(&source, format!("binary:{binary}")).expect("write fake binary");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755))
            .expect("set fake binary mode");
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
        "unixnotis-css-validate",
        "noticenterctl",
    ] {
        let installed = paths.bin_dir.join(binary);
        assert!(installed.exists(), "{binary} should be installed");
        assert_eq!(
            fs::read_to_string(&installed).expect("read installed binary"),
            format!("binary:{binary}")
        );
        assert_eq!(
            fs::metadata(&installed)
                .expect("installed binary metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755
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
        "unixnotis-css-validate",
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
fn install_binaries_rejects_destination_symlink_without_touching_its_target() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("install-binaries-temp-symlink");
    write_fake_workspace(
        &root,
        &[
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "unixnotis-css-validate",
            "noticenterctl",
        ],
    );
    let paths = test_paths(&root);
    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "unixnotis-css-validate",
        "noticenterctl",
    ] {
        let source = paths.repo_root.join("target").join("release").join(binary);
        fs::create_dir_all(source.parent().expect("release dir")).expect("make release dir");
        fs::write(&source, format!("binary:{binary}")).expect("write fake binary");
    }
    fs::create_dir_all(&paths.bin_dir).expect("bin dir");
    let destination = paths.bin_dir.join("unixnotis-daemon");
    let protected = root.join("protected");
    fs::write(&protected, "protected").expect("protected");
    symlink(&protected, &destination).expect("destination symlink");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);

    let error = install_binaries(&mut ctx).expect_err("destination symlink should fail");

    assert!(error.to_string().contains("failed to install"));
    assert_eq!(
        fs::read_to_string(&protected).expect("protected remains"),
        "protected"
    );
    assert!(fs::symlink_metadata(&destination)
        .expect("destination symlink remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_binaries_removes_all_managed_binaries_and_runtime_helpers() {
    // Uninstall must remove the same managed set that install copied in
    let root = test_root("remove-binaries");
    write_fake_workspace(
        &root,
        &[
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "unixnotis-css-validate",
            "noticenterctl",
        ],
    );
    let paths = test_paths(&root);

    fs::create_dir_all(&paths.bin_dir).expect("make bin dir");
    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "unixnotis-css-validate",
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
        "unixnotis-css-validate",
        "noticenterctl",
    ] {
        assert!(
            !paths.bin_dir.join(binary).exists(),
            "{binary} should be removed"
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn remove_binaries_rejects_symlink_without_touching_its_target() {
    let root = test_root("remove-binaries-symlink");
    write_fake_workspace(&root, &["unixnotis-daemon"]);
    let paths = test_paths(&root);
    let protected = root.join("protected");
    let installed = paths.bin_dir.join("unixnotis-daemon");
    fs::create_dir_all(&paths.bin_dir).expect("create bin directory");
    fs::write(&protected, "protected").expect("write protected file");
    symlink(&protected, &installed).expect("create installed binary link");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);

    remove_binaries(&mut ctx).expect_err("binary link should be rejected");

    assert_eq!(
        fs::read_to_string(&protected).expect("read protected file"),
        "protected"
    );
    assert!(fs::symlink_metadata(&installed)
        .expect("installed link remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn remove_binaries_never_removes_a_file_outside_the_bin_directory() {
    let root = test_root("remove-binaries-contained");
    let paths = test_paths(&root);
    fs::create_dir_all(&paths.repo_root).expect("workspace root");
    fs::write(
        paths.repo_root.join("Cargo.toml"),
        r#"
[workspace]
members = []

[workspace.metadata.unixnotis.installer]
binaries = ["../../.bashrc"]
"#,
    )
    .expect("crafted workspace metadata");
    let sentinel = root.join("home").join(".bashrc");
    fs::create_dir_all(sentinel.parent().expect("sentinel parent")).expect("home directory");
    fs::write(&sentinel, "keep this file").expect("outside sentinel");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);

    remove_binaries(&mut ctx).expect("fallback uninstall should stay contained");

    assert_eq!(
        fs::read_to_string(&sentinel).expect("outside sentinel remains"),
        "keep this file"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn resolved_binary_removal_rejects_traversal_before_touching_an_outside_file() {
    let root = test_root("remove-resolved-binaries-contained");
    let paths = test_paths(&root);
    fs::create_dir_all(&paths.bin_dir).expect("bin directory");
    let sentinel = root.join("home").join("sentinel");
    fs::write(&sentinel, "keep this file").expect("outside sentinel");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);

    let error = remove_resolved_binaries(&mut ctx, vec!["../../sentinel".to_string()])
        .expect_err("removal must reject a path that escapes the bin directory");

    assert!(error.to_string().contains("unmanaged binary path"));
    assert_eq!(
        fs::read_to_string(&sentinel).expect("outside sentinel remains"),
        "keep this file"
    );
    let _ = fs::remove_dir_all(&root);
}

fn write_fake_release_archive(root: &std::path::Path) {
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("release bin dir");
    fs::write(
        root.join("unixnotis-release.json"),
        r#"{"version":"1.0.0","binaries":["unixnotis-daemon","unixnotis-popups","unixnotis-center","unixnotis-css-validate","noticenterctl"]}"#,
    )
    .expect("release manifest");

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "unixnotis-css-validate",
        "noticenterctl",
    ] {
        fs::write(bin_dir.join(binary), format!("release:{binary}")).expect("release binary");
    }
}
