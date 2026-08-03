use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::super::{reset_config_to_defaults, snapshot_existing_file, ResetConfigOptions};
use super::support::temp_config_dir;
use crate::DEFAULT_SCRIPTS;

#[test]
fn reset_backs_up_existing_files_and_writes_stock_files() {
    let root = temp_config_dir("present");
    fs::write(root.join("config.toml"), "custom = true\n").expect("seed config");
    let script = root.join(DEFAULT_SCRIPTS[0].relative_path);
    fs::create_dir_all(script.parent().expect("script parent")).expect("script directory");
    fs::write(&script, "custom script\n").expect("seed script");

    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: root.clone(),
        backup_retention: 3,
    })
    .expect("reset should succeed");

    let config_text = fs::read_to_string(root.join("config.toml")).expect("read reset config");
    toml::from_str::<crate::Config>(&config_text).expect("reset config should parse");
    assert_eq!(
        fs::read_to_string(&script).expect("read reset script"),
        DEFAULT_SCRIPTS[0].contents
    );
    let backup = report.backup_dir.expect("backup directory");
    assert_eq!(
        fs::read_to_string(backup.join("config.toml")).expect("read config backup"),
        "custom = true\n"
    );
    assert_eq!(
        fs::read_to_string(backup.join("unixnotis-blue-light-lib")).expect("read script backup"),
        "custom script\n"
    );
    assert_eq!(report.written_files.len(), 1 + DEFAULT_SCRIPTS.len());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reset_creates_missing_config_and_scripts_with_safe_modes() {
    let root = temp_config_dir("missing");
    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: root.clone(),
        backup_retention: 0,
    })
    .expect("reset should create missing files");
    assert!(report.backup_dir.is_none());
    assert!(root.join("config.toml").is_file());
    for script in DEFAULT_SCRIPTS {
        let path = root.join(script.relative_path);
        assert!(path.is_file());
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(path)
                .expect("script metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reset_preserves_custom_theme_files_but_backs_them_up() {
    let root = temp_config_dir("theme");
    fs::write(root.join("panel.css"), "custom panel\n").expect("seed custom CSS");
    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: root.clone(),
        backup_retention: 1,
    })
    .expect("reset should succeed");
    assert_eq!(
        fs::read_to_string(root.join("panel.css")).expect("read custom CSS"),
        "custom panel\n"
    );
    let backup = report.backup_dir.expect("backup directory");
    assert_eq!(
        fs::read_to_string(backup.join("panel.css")).expect("read CSS backup"),
        "custom panel\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reset_accepts_large_existing_files_and_backs_them_up() {
    let root = temp_config_dir("large-file");
    let original = vec![b'x'; 8 * 1024 * 1024 + 1];
    fs::write(root.join("config.toml"), &original).expect("seed large config");

    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: root.clone(),
        backup_retention: 1,
    })
    .expect("the configured boundary remains valid");

    let backup = report.backup_dir.expect("boundary backup directory");
    assert_eq!(
        fs::read(backup.join("config.toml")).expect("read large backup"),
        original
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn snapshot_reports_errors_other_than_missing_files() {
    let root = temp_config_dir("snapshot-error");
    let parent_file = root.join("not-a-directory");
    fs::write(&parent_file, b"file").expect("seed parent file");

    let error = snapshot_existing_file(&parent_file.join("child"))
        .expect_err("a non-directory parent must not look like a missing file");
    assert!(error.to_string().contains("inspect"), "{error}");
    let _ = fs::remove_dir_all(root);
}
