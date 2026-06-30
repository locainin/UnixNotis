use super::*;
use crate::test_support::env_lock;

fn test_executable_dir() -> PathBuf {
    std::env::current_exe()
        .expect("current test executable")
        .parent()
        .expect("test executable should have a parent")
        .to_path_buf()
}

fn write_sibling(name: &str) -> PathBuf {
    let path = test_executable_dir().join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write sibling binary");
    path
}

#[test]
fn resolve_sibling_binary_prefers_exact_sibling_name() {
    let _guard = env_lock();
    let path = write_sibling("unixnotis-popups");

    assert_eq!(
        resolve_sibling_binary("unixnotis-popups"),
        Some(path.clone())
    );
    assert_eq!(resolve_popups_path(), Some(path.clone()));

    let _ = std::fs::remove_file(path);
}

#[test]
fn resolve_sibling_binary_falls_back_to_exe_suffix() {
    let _guard = env_lock();
    let path = write_sibling("unixnotis-center.exe");
    let exact = test_executable_dir().join("unixnotis-center");
    let _ = std::fs::remove_file(&exact);

    assert_eq!(
        resolve_sibling_binary("unixnotis-center"),
        Some(path.clone())
    );
    assert_eq!(resolve_center_path(), Some(path.clone()));

    let _ = std::fs::remove_file(path);
}

#[test]
fn resolve_sibling_binary_returns_none_when_no_sibling_exists() {
    let _guard = env_lock();
    let exact = test_executable_dir().join("unixnotis-missing");
    let exe = test_executable_dir().join("unixnotis-missing.exe");
    let _ = std::fs::remove_file(&exact);
    let _ = std::fs::remove_file(&exe);

    assert!(resolve_sibling_binary("unixnotis-missing").is_none());
}
