use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::test_support::env::EnvGuard;
use crate::test_support::fs::unique_temp_path;
use unixnotis_core::util::TRUSTED_SYSTEM_TOOL_DIRS;

use super::{account_home_dir, cargo_command, resolve_cargo};

#[test]
fn cargo_resolution_ignores_poisoned_home_and_path_entries() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = unique_temp_path("cargo-poisoned-path");
    let poisoned_home = root.join("home");
    let poisoned_home_cargo = poisoned_home.join(".cargo/bin/cargo");
    let poisoned = root.join("poisoned/cargo");
    fs::create_dir_all(poisoned_home_cargo.parent().expect("poisoned HOME parent"))
        .expect("poisoned HOME directory");
    fs::create_dir_all(poisoned.parent().expect("poisoned parent")).expect("poisoned directory");
    write_direct_executable(&poisoned_home_cargo, "#!/bin/sh\nexit 41\n");
    write_direct_executable(&poisoned, "#!/bin/sh\nexit 42\n");

    let _home = EnvGuard::set("HOME", &poisoned_home);
    let _path = EnvGuard::set("PATH", poisoned.parent().expect("poisoned parent"));

    let resolved = resolve_cargo().expect("Cargo should resolve from the account home");

    assert!(resolved.is_absolute());
    assert_eq!(
        resolved,
        fs::canonicalize(&resolved).expect("resolved path canonicalized")
    );
    assert!(!resolved.starts_with(&poisoned_home));
    assert_ne!(
        resolved,
        fs::canonicalize(poisoned).expect("poisoned path canonicalized")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cargo_command_removes_compiler_override_environment() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = unique_temp_path("cargo-command-environment");
    fs::create_dir_all(&root).expect("create command root");
    let poisoned_path = poisoned_parent(&root);
    fs::create_dir_all(&poisoned_path).expect("create poisoned PATH directory");
    let _environment = set_poisoned_cargo_environment(&root, &poisoned_path);

    let cargo = resolve_cargo().expect("trusted Cargo should resolve");
    let command = cargo_command(&cargo).expect("Cargo command should be configured");

    assert_eq!(command.get_program(), cargo.as_os_str());
    assert_compiler_environment_is_pinned(&command);
    assert_path_environment_is_sanitized(&command, &poisoned_path);
    assert_account_paths_are_pinned(&command);

    let _ = fs::remove_dir_all(root);
}

fn set_poisoned_cargo_environment(root: &Path, poisoned_path: &Path) -> Vec<EnvGuard> {
    vec![
        EnvGuard::set("HOME", root.join("attacker-home")),
        EnvGuard::set("PATH", poisoned_path),
        EnvGuard::set("RUSTC", "/tmp/attacker-rustc"),
        EnvGuard::set("RUSTDOC", "/tmp/attacker-rustdoc"),
        EnvGuard::set("RUSTC_WRAPPER", "/tmp/attacker-wrapper"),
        EnvGuard::set("RUSTC_WORKSPACE_WRAPPER", "/tmp/attacker-workspace-wrapper"),
        EnvGuard::set("CARGO_BUILD_RUSTC_WRAPPER", "/tmp/attacker-cargo-wrapper"),
        EnvGuard::set(
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
            "/tmp/attacker-cargo-workspace-wrapper",
        ),
        EnvGuard::set("RUSTUP_TOOLCHAIN", "/tmp/attacker-toolchain"),
        EnvGuard::set("RUSTFLAGS", "--cfg attacker"),
        EnvGuard::set("CARGO_ENCODED_RUSTFLAGS", "--cfg\u{1f}attacker"),
        EnvGuard::set("CARGO_BUILD_RUSTFLAGS", "--cfg attacker-build"),
        EnvGuard::set("CARGO_BUILD_ENCODED_RUSTFLAGS", "--cfg\u{1f}attacker-build"),
        EnvGuard::set(
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
            "/tmp/attacker-linker",
        ),
        EnvGuard::set(
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER",
            "/tmp/attacker-runner",
        ),
        EnvGuard::set(
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
            "--cfg attacker-target",
        ),
    ]
}

fn assert_compiler_environment_is_pinned(command: &Command) {
    for variable in [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTDOC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_BUILD_ENCODED_RUSTFLAGS",
        "RUSTUP_TOOLCHAIN",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
    ] {
        assert!(
            command
                .get_envs()
                .find(|(name, _value)| *name == OsStr::new(variable))
                .is_some_and(|(_name, value)| value.is_none()),
            "{variable} must be removed from the Cargo environment"
        );
    }

    for variable in ["RUSTC", "RUSTDOC"] {
        let value = command_env(command, variable)
            .unwrap_or_else(|| panic!("{variable} must be pinned in the Cargo environment"));
        let path = Path::new(value);
        assert!(path.is_absolute(), "{variable} must be absolute");
        assert!(path.is_file(), "{variable} must name an executable");
        assert!(
            fs::canonicalize(path)
                .expect("compiler path should canonicalize")
                .is_file(),
            "{variable} canonical target must be a file"
        );
    }
}

fn assert_path_environment_is_sanitized(command: &Command, poisoned_path: &Path) {
    let path_value =
        command_env(command, "PATH").expect("Cargo PATH should be explicitly replaced");
    let path_dirs = std::env::split_paths(path_value).collect::<Vec<_>>();
    assert!(!path_dirs.iter().any(|path| {
        path.as_path() == poisoned_path
            || path
                .file_name()
                .is_some_and(|name| name == OsStr::new("attacker-home"))
    }));
    let expected_system_dirs = TRUSTED_SYSTEM_TOOL_DIRS
        .iter()
        .filter_map(|directory| fs::canonicalize(directory).ok())
        .fold(Vec::new(), |mut directories, directory| {
            if !directories.contains(&directory) {
                directories.push(directory);
            }
            directories
        });
    assert!(path_dirs.ends_with(&expected_system_dirs));

    let rustc_directory = fs::canonicalize(
        Path::new(command_env(command, "RUSTC").expect("RUSTC should be present"))
            .parent()
            .expect("RUSTC should have a parent directory"),
    )
    .expect("RUSTC parent should be canonicalized");
    assert!(
        path_dirs.contains(&rustc_directory),
        "PATH should contain the validated rustc directory"
    );
}

fn assert_account_paths_are_pinned(command: &Command) {
    let account_home = account_home_dir().expect("effective account home");
    assert_eq!(command_env(command, "HOME"), Some(account_home.as_os_str()));
    assert_eq!(
        command_env(command, "CARGO_HOME"),
        Some(account_home.join(".cargo").as_os_str())
    );
    assert_eq!(
        command_env(command, "RUSTUP_HOME"),
        Some(account_home.join(".rustup").as_os_str())
    );
}

fn command_env<'a>(command: &'a Command, variable: &str) -> Option<&'a OsStr> {
    command
        .get_envs()
        .find(|(name, _value)| *name == OsStr::new(variable))
        .and_then(|(_name, value)| value)
}

fn write_direct_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("set executable mode");
}

fn poisoned_parent(root: &Path) -> PathBuf {
    root.join("attacker-path")
}
