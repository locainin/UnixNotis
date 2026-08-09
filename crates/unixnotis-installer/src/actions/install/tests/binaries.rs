use std::fs;

use crate::detect::Detection;
use crate::model::ActionMode;

use super::super::binaries::remove_resolved_binaries;
use super::super::binaries::{log_installed_generation, resolve_install_inputs};
use super::super::remove_binaries;
use super::support::{test_context, test_paths, test_root, write_fake_workspace};

fn install_binaries_with_guards<F, R, G>(
    ctx: &mut crate::actions::ActionContext,
    mut precommit: F,
    mut reserve_activation: R,
) -> anyhow::Result<()>
where
    F: FnMut(&crate::paths::InstallPaths) -> anyhow::Result<()>,
    R: FnMut(&crate::paths::InstallPaths) -> anyhow::Result<G>,
{
    let (binaries, release_dir) = resolve_install_inputs(ctx)?;
    let generation = crate::actions::releases::install_release_generation_transaction(
        ctx.paths,
        &release_dir,
        &binaries,
        || precommit(ctx.paths),
        || reserve_activation(ctx.paths),
        || Ok(()),
    )?;
    log_installed_generation(ctx, &binaries, &generation);
    Ok(())
}

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

    install_binaries_with_guards(&mut ctx, |_paths| Ok(()), |_paths| Ok(()))
        .expect("install should copy binaries");

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

    install_binaries_with_guards(&mut ctx, |_paths| Ok(()), |_paths| Ok(()))
        .expect("release archive install should copy binaries");

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

#[test]
fn binary_install_runs_the_live_precommit_gate_and_activates_the_release() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("install-binaries-public-boundary");
    write_fake_workspace(&root, &["unixnotis-daemon"]);
    let paths = test_paths(&root);
    let source = paths
        .repo_root
        .join("target")
        .join("release")
        .join("unixnotis-daemon");
    fs::create_dir_all(source.parent().expect("release source parent"))
        .expect("create release source directory");
    fs::write(&source, "public boundary binary").expect("write release source");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o755))
        .expect("set release source mode");
    let fake_bin = root.join("fake-tools");
    fs::create_dir_all(&fake_bin).expect("create fake tools directory");
    crate::test_support::fs::write_executable(
        &fake_bin.join("busctl"),
        "#!/bin/sh\nprintf 'b false\\n'\n",
    );
    crate::test_support::fs::write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\nprintf 'LoadState=not-found\\nActiveState=inactive\\n'\n",
    );
    let _system_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let _manager_tools =
        crate::service_manager::contract::command_routing::use_fake_command_bin(&fake_bin);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);

    install_binaries_with_guards(
        &mut ctx,
        |paths| {
            crate::actions::daemon::wait_until_no_conflicting_live_daemon(
                paths,
                crate::actions::daemon::STOP_QUIESCENCE_TIMEOUT,
            )
        },
        |_paths| Ok(()),
    )
    .expect("binary install should activate one generation after the live gate");

    assert_eq!(
        fs::read_to_string(paths.bin_dir.join("unixnotis-daemon")).expect("read installed binary"),
        "public boundary binary"
    );
    fs::remove_dir_all(root).expect("remove public install fixture");
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
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755))
            .expect("set fake binary mode");
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

    let error = install_binaries_with_guards(&mut ctx, |_paths| Ok(()), |_paths| Ok(()))
        .expect_err("destination symlink should fail");

    assert!(
        error.to_string().contains("unmanaged target"),
        "unexpected destination error: {error:#}"
    );
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

#[test]
fn remove_binaries_accepts_only_the_managed_generation_entrypoints() {
    let root = test_root("remove-managed-generation-binaries");
    write_fake_workspace(&root, &["unixnotis-daemon", "unixnotis-center"]);
    let paths = test_paths(&root);
    for binary in ["unixnotis-daemon", "unixnotis-center"] {
        let source = paths.repo_root.join("target").join("release").join(binary);
        fs::create_dir_all(source.parent().expect("release source parent"))
            .expect("create release source directory");
        fs::write(&source, format!("managed:{binary}")).expect("write release source");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755))
            .expect("set release source mode");
    }
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);
    install_binaries_with_guards(&mut ctx, |_paths| Ok(()), |_paths| Ok(()))
        .expect("install managed generation");

    remove_binaries(&mut ctx).expect("remove managed generation");

    assert!(fs::symlink_metadata(paths.bin_dir.join("unixnotis-daemon")).is_err());
    assert!(fs::symlink_metadata(paths.bin_dir.join("unixnotis-center")).is_err());
    assert!(!paths
        .installed_release_root()
        .expect("installed release root")
        .exists());
    fs::remove_dir_all(root).expect("remove managed uninstall fixture");
}

#[test]
fn remove_binaries_rejects_a_directory_entrypoint_with_a_stable_error() {
    let root = test_root("remove-binaries-directory-entrypoint");
    write_fake_workspace(&root, &["unixnotis-daemon"]);
    let paths = test_paths(&root);
    fs::create_dir_all(paths.bin_dir.join("unixnotis-daemon"))
        .expect("create directory entrypoint");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);

    let error = remove_binaries(&mut ctx).expect_err("directory entrypoint must fail closed");

    assert!(error.to_string().contains("non-file binary entrypoint"));
    assert!(paths.bin_dir.join("unixnotis-daemon").is_dir());
    fs::remove_dir_all(root).expect("remove directory entrypoint fixture");
}

#[test]
fn resolved_binary_removal_propagates_entrypoint_inspection_errors() {
    let root = test_root("remove-binaries-inspection-error");
    let paths = test_paths(&root);
    fs::create_dir_all(paths.bin_dir.parent().expect("binary parent"))
        .expect("create binary parent");
    fs::write(&paths.bin_dir, "not a directory").expect("create invalid binary root");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);

    let error = remove_resolved_binaries(&mut ctx, vec!["unixnotis-daemon".to_string()])
        .expect_err("entrypoint metadata errors must not become missing files");

    assert!(error.to_string().contains("inspect"));
    fs::remove_file(&paths.bin_dir).expect("remove invalid binary root");
    fs::remove_dir_all(root).expect("remove inspection error fixture");
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
        let path = bin_dir.join(binary);
        fs::write(&path, format!("release:{binary}")).expect("release binary");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("set release binary mode");
    }
}
