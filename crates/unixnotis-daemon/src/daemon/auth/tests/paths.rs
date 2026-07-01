use super::filesystem::canonicalize_best_effort;
use super::paths::{
    is_trusted_control_executable_path_relaxed_in_dir, trusted_local_bin_matches_executable,
    trusted_path_matches_executable, trusted_profile_sibling_matches_executable,
};
use super::support::write_executable;
use crate::test_support::{env_lock, EnvVarGuard, TempRoot};

#[test]
fn trusted_path_match_requires_exact_canonical_sibling() {
    let root = TempRoot::new("auth-trusted-path");
    let trusted = root.join("noticenterctl");
    let outsider = root.join("other").join("noticenterctl");
    write_executable(&trusted);
    write_executable(&outsider);

    assert!(trusted_path_matches_executable(
        root.path(),
        "noticenterctl",
        &canonicalize_best_effort(&trusted)
    ));
    assert!(!trusted_path_matches_executable(
        root.path(),
        "noticenterctl",
        &canonicalize_best_effort(&outsider)
    ));
}

#[test]
fn trusted_profile_sibling_requires_debug_or_release_target_root() {
    let target = TempRoot::new("auth-profile-root");
    let debug_dir = target.join("debug");
    let release_dir = target.join("release");
    let build_dir = target.join("build");
    let release_ctl = release_dir.join("noticenterctl");
    let outsider = target.join("other").join("noticenterctl");
    write_executable(&release_ctl);
    write_executable(&outsider);

    assert!(trusted_profile_sibling_matches_executable(
        &debug_dir,
        "noticenterctl",
        &canonicalize_best_effort(&release_ctl)
    ));
    assert!(!trusted_profile_sibling_matches_executable(
        &build_dir,
        "noticenterctl",
        &canonicalize_best_effort(&release_ctl)
    ));
    assert!(!trusted_profile_sibling_matches_executable(
        &debug_dir,
        "noticenterctl",
        &canonicalize_best_effort(&outsider)
    ));
}

#[test]
fn trusted_local_bin_uses_home_local_bin_exactly() {
    let _guard = env_lock();
    let home = TempRoot::new("auth-home");
    let local_ctl = home.join(".local/bin/noticenterctl");
    let wrong_name = home.join(".local/bin/untrusted");
    let outside = home.join("bin/noticenterctl");
    write_executable(&local_ctl);
    write_executable(&wrong_name);
    write_executable(&outside);
    let _home = EnvVarGuard::set("HOME", home.path());

    assert!(trusted_local_bin_matches_executable(
        "noticenterctl",
        &canonicalize_best_effort(&local_ctl)
    ));
    assert!(!trusted_local_bin_matches_executable(
        "noticenterctl",
        &canonicalize_best_effort(&outside)
    ));
    assert!(!trusted_local_bin_matches_executable(
        "noticenterctl",
        &canonicalize_best_effort(&wrong_name)
    ));
}

#[test]
fn trusted_local_bin_requires_home() {
    let _guard = env_lock();
    let root = TempRoot::new("auth-no-home");
    let local_ctl = root.join(".local/bin/noticenterctl");
    write_executable(&local_ctl);
    let _home = EnvVarGuard::remove("HOME");

    assert!(!trusted_local_bin_matches_executable(
        "noticenterctl",
        &canonicalize_best_effort(&local_ctl)
    ));
}

#[test]
fn relaxed_path_check_accepts_safe_trusted_sibling() {
    let root = TempRoot::new("auth-relaxed-sibling");
    let trusted = root.join("noticenterctl");
    write_executable(&trusted);

    assert!(is_trusted_control_executable_path_relaxed_in_dir(
        &canonicalize_best_effort(&trusted),
        root.path()
    ));
}

#[test]
fn relaxed_path_check_accepts_safe_profile_sibling() {
    let target = TempRoot::new("auth-relaxed-profile");
    let trusted_dir = target.join("debug");
    let release_dir = target.join("release");
    let release_ctl = release_dir.join("noticenterctl");
    std::fs::create_dir_all(&trusted_dir).expect("debug dir");
    write_executable(&release_ctl);

    assert!(is_trusted_control_executable_path_relaxed_in_dir(
        &canonicalize_best_effort(&release_ctl),
        &trusted_dir
    ));
}

#[test]
fn relaxed_path_check_rejects_unknown_name_and_missing_file() {
    let root = TempRoot::new("auth-relaxed-name");
    let wrong_name = root.join("not-unixnotis");
    let missing = root.join("noticenterctl");
    write_executable(&wrong_name);

    assert!(!is_trusted_control_executable_path_relaxed_in_dir(
        &canonicalize_best_effort(&wrong_name),
        root.path()
    ));
    assert!(!is_trusted_control_executable_path_relaxed_in_dir(
        &missing,
        root.path()
    ));
}

#[test]
fn relaxed_path_check_rejects_directory_even_with_allowed_name() {
    let root = TempRoot::new("auth-relaxed-dir");
    let directory = root.join("noticenterctl");
    std::fs::create_dir_all(&directory).expect("trusted-name directory");

    assert!(!is_trusted_control_executable_path_relaxed_in_dir(
        &directory,
        root.path()
    ));
}

#[cfg(unix)]
#[test]
fn relaxed_path_check_rejects_group_writable_file() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempRoot::new("auth-relaxed-mode");
    let trusted = root.join("noticenterctl");
    write_executable(&trusted);
    let mut permissions = std::fs::metadata(&trusted).expect("metadata").permissions();
    permissions.set_mode(0o775);
    std::fs::set_permissions(&trusted, permissions).expect("set unsafe mode");

    assert!(!is_trusted_control_executable_path_relaxed_in_dir(
        &trusted,
        root.path()
    ));
}
