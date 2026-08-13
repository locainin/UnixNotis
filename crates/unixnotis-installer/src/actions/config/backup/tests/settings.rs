//! Installer settings file tests

use std::fs;

use crate::detect::Detection;

use super::super::settings::ensure_installer_config;
use super::support::{test_context, test_paths};

#[test]
fn installer_config_is_created_once_and_preserves_existing_settings() {
    let root = crate::test_support::fs::unique_temp_path("installer-settings");
    let config_dir = root.join("config");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);

    let config_path = ensure_installer_config(&mut context, &config_dir)
        .expect("installer config should be created");

    assert_eq!(config_path, config_dir.join("installer.toml"));
    assert_eq!(
        fs::read_to_string(&config_path).expect("read installer config"),
        "# UnixNotis installer settings\n# Backup retention for config/theme resets\n[backups]\nkeep = 3\n"
    );

    fs::write(&config_path, "[backups]\nkeep = 9\n").expect("customize installer config");
    let retained = ensure_installer_config(&mut context, &config_dir)
        .expect("existing installer config should be retained");

    assert_eq!(retained, config_path);
    assert_eq!(
        fs::read_to_string(&retained).expect("read retained installer config"),
        "[backups]\nkeep = 9\n"
    );
    let _ = fs::remove_dir_all(root);
}
