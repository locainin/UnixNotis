use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::super::{reset_config_to_defaults_with_writer, ResetConfigOptions};
use super::support::temp_config_dir;

#[test]
fn reset_restores_replaced_files_after_a_later_write_fails() {
    let root = temp_config_dir("rollback");
    let config_path = root.join("config.toml");
    let script_path = root.join("scripts/unixnotis-blue-light-lib");
    fs::write(&config_path, "original config\n").expect("seed config");
    #[cfg(unix)]
    fs::set_permissions(&config_path, fs::Permissions::from_mode(0o640))
        .expect("set original config mode");
    fs::create_dir_all(script_path.parent().expect("script parent")).expect("script directory");
    fs::write(&script_path, "original script\n").expect("seed script");
    let failure_path = script_path.clone();

    let error = reset_config_to_defaults_with_writer(
        &ResetConfigOptions {
            config_dir: root.clone(),
            backup_retention: 1,
        },
        &move |path, contents, mode| {
            if path == failure_path.as_path() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected publication failure",
                ));
            }
            crate::filesystem::write_file_atomic(path, contents, mode)
        },
    )
    .expect_err("the injected failure must be returned");

    assert!(error.to_string().contains("injected publication failure"));
    assert_eq!(
        fs::read_to_string(&config_path).expect("restored config"),
        "original config\n"
    );
    assert_eq!(
        fs::read_to_string(script_path).expect("original script"),
        "original script\n"
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(config_path)
            .expect("restored config metadata")
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollback_attempts_every_written_path_and_reports_all_failures() {
    let root = temp_config_dir("rollback-all");
    let config_path = root.join("config.toml");
    let first_script = root.join("scripts/unixnotis-blue-light-state");
    let second_script = root.join("scripts/unixnotis-blue-light-on");
    fs::write(&config_path, "original config\n").expect("seed config");
    fs::create_dir_all(first_script.parent().expect("script parent")).expect("script directory");
    fs::write(&first_script, "original state\n").expect("seed first script");
    fs::write(&second_script, "original on\n").expect("seed second script");
    let error = reset_config_to_defaults_with_writer(
        &ResetConfigOptions {
            config_dir: root.clone(),
            backup_retention: 1,
        },
        &move |path, contents, mode| {
            if path == second_script && contents != b"original on\n" {
                return Err(io::Error::other("injected publication failure"));
            }
            if path == first_script && contents == b"original state\n" {
                return Err(io::Error::other("injected rollback failure"));
            }
            crate::filesystem::write_file_atomic(path, contents, mode)
        },
    )
    .expect_err("the injected failure must be returned");

    let message = error.to_string();
    assert!(message.contains("injected publication failure"));
    assert!(message.contains("rollback failed"), "{message}");
    assert!(message.contains("injected rollback failure"), "{message}");
    assert_eq!(
        fs::read_to_string(config_path).expect("config rollback should be attempted"),
        "original config\n"
    );
    let _ = fs::remove_dir_all(root);
}
