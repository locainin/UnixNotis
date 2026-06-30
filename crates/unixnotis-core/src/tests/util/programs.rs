use std::fs;

use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn executable_path_accepts_only_regular_executable_files() {
    let path = crate::test_support::unique_temp_path("executable-file");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("test executable should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("test executable permissions should be writable");

    assert!(is_executable_path(&path));

    let _ = fs::remove_file(path);
}

#[test]
fn executable_path_rejects_non_executable_regular_files() {
    let path = crate::test_support::unique_temp_path("plain-file");
    fs::write(&path, "not executable").expect("test file should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("test file permissions should be writable");

    #[cfg(unix)]
    assert!(!is_executable_path(&path));

    let _ = fs::remove_file(path);
}

#[test]
fn executable_path_rejects_directories_and_missing_paths() {
    let dir = crate::test_support::unique_temp_path("dir");
    fs::create_dir(&dir).expect("test directory should be writable");
    let missing = dir.join("missing");

    assert!(!is_executable_path(&dir));
    assert!(!is_executable_path(&missing));

    let _ = fs::remove_dir(dir);
}

#[test]
fn program_lookup_with_explicit_path_uses_executable_rules() {
    let path = crate::test_support::unique_temp_path("explicit-program");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("test program should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("test program permissions should be writable");

    assert!(program_in_path(path.to_string_lossy().as_ref()));

    let _ = fs::remove_file(path);
}

#[test]
fn program_lookup_with_explicit_path_rejects_non_executable_file() {
    let path = crate::test_support::unique_temp_path("explicit-non-executable");
    fs::write(&path, "plain").expect("test program should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("test program permissions should be writable");

    #[cfg(unix)]
    assert!(!program_in_path(path.to_string_lossy().as_ref()));

    let _ = fs::remove_file(path);
}

#[test]
fn program_lookup_cache_refreshes_when_path_changes() {
    let _guard = crate::test_support::test_env_lock();
    let root = crate::test_support::unique_temp_path("path-cache");
    let bin_a = root.join("a");
    let bin_b = root.join("b");
    fs::create_dir_all(&bin_a).expect("first bin dir");
    fs::create_dir_all(&bin_b).expect("second bin dir");
    let program = format!("unixnotis-test-tool-{}", std::process::id());
    let first = bin_a.join(&program);
    let second = bin_b.join(&program);
    fs::write(&first, "#!/bin/sh\nexit 0\n").expect("first program");
    fs::write(&second, "#!/bin/sh\nexit 0\n").expect("second program");

    #[cfg(unix)]
    {
        fs::set_permissions(&first, fs::Permissions::from_mode(0o755))
            .expect("first program permissions");
        fs::set_permissions(&second, fs::Permissions::from_mode(0o755))
            .expect("second program permissions");
    }

    let _path = crate::test_support::EnvGuard::set("PATH", bin_a.as_os_str());
    assert!(program_in_path(&program));

    // Removing the first file leaves a stale true result unless PATH changes clear the cache
    fs::remove_file(&first).expect("remove first program");
    std::env::set_var("PATH", bin_b.to_string_lossy().as_ref());
    assert!(program_in_path(&program));

    // A new PATH with no executable must also clear a cached true result
    std::env::set_var("PATH", root.to_string_lossy().as_ref());
    assert!(!program_in_path(&program));

    let _ = fs::remove_dir_all(root);
}
