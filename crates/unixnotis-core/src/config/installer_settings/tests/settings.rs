use super::super::{ensure_installer_config, load_installer_config};

fn test_directory(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "unixnotis-installer-settings-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create settings directory");
    path
}

#[test]
fn ensure_creates_shared_defaults_once() {
    let directory = test_directory("create");
    let (path, created) = ensure_installer_config(&directory).expect("create settings");
    assert!(created);
    assert!(path.is_file());

    let settings = load_installer_config(&directory).expect("load settings");
    assert_eq!(settings.backups.keep, 3);

    let (_, created_again) = ensure_installer_config(&directory).expect("keep existing settings");
    assert!(!created_again);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn load_preserves_existing_retention() {
    let directory = test_directory("load");
    std::fs::write(directory.join("installer.toml"), "[backups]\nkeep = 9\n")
        .expect("write settings");

    let settings = load_installer_config(&directory).expect("load settings");
    assert_eq!(settings.backups.keep, 9);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn load_missing_settings_uses_defaults() {
    let directory = test_directory("missing");

    let settings = load_installer_config(&directory).expect("missing settings use defaults");

    assert_eq!(settings.backups.keep, 3);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn load_invalid_settings_fails_instead_of_defaulting() {
    let directory = test_directory("invalid");
    std::fs::write(directory.join("installer.toml"), "[backups\n").expect("write invalid settings");

    let error = load_installer_config(&directory).expect_err("invalid settings must fail");

    assert!(error.to_string().contains("parse"));
    let _ = std::fs::remove_dir_all(directory);
}

#[cfg(unix)]
#[test]
fn load_directory_settings_fails_instead_of_defaulting() {
    let directory = test_directory("directory");
    std::fs::create_dir(directory.join("installer.toml")).expect("create invalid settings path");

    let error = load_installer_config(&directory).expect_err("directory settings must fail");

    assert!(error.to_string().contains("read"));
    let _ = std::fs::remove_dir_all(directory);
}

#[cfg(unix)]
#[test]
fn ensure_rejects_a_settings_symlink_without_replacing_it() {
    let directory = test_directory("symlink");
    let target = directory.join("settings-target");
    std::fs::write(&target, b"[backups]\nkeep = 7\n").expect("write target settings");
    std::os::unix::fs::symlink(&target, directory.join("installer.toml"))
        .expect("create settings symlink");

    let error = ensure_installer_config(&directory).expect_err("symlink must be rejected");

    assert!(error.to_string().contains("write installer settings"));
    assert_eq!(
        std::fs::read_to_string(target).expect("read target settings"),
        "[backups]\nkeep = 7\n"
    );
    let _ = std::fs::remove_dir_all(directory);
}
