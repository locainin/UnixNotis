//! Tests for provisioning built-in helper scripts

use std::fs;

use crate::Config;

use super::support::test_root;

#[test]
fn ensure_default_scripts_in_creates_every_shipped_script() {
    let root = test_root("default-scripts");
    // Start clean so every file must be created by the helper under test
    let _ = fs::remove_dir_all(&root);

    Config::ensure_default_scripts_in(&root).expect("default scripts");

    for script in crate::DEFAULT_SCRIPTS {
        let path = root.join(script.relative_path);
        // Shipped script contents should be byte-for-byte stable
        let contents = fs::read_to_string(&path).expect("read default script");
        assert_eq!(contents, script.contents);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&path)
                .expect("script metadata")
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "script should be executable: {path:?}");
        }
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ensure_default_scripts_in_preserves_user_edited_script_contents() {
    let root = test_root("default-script-preserve");
    // Existing scripts represent user edits and should not be overwritten by ensure
    let _ = fs::remove_dir_all(&root);
    let script = crate::DEFAULT_SCRIPTS
        .iter()
        .find(|script| script.relative_path.ends_with("unixnotis-blue-light-on"))
        .expect("blue light on script");
    let path = root.join(script.relative_path);
    fs::create_dir_all(path.parent().expect("script parent")).expect("script parent dir");
    fs::write(&path, "#!/bin/sh\nexit 42\n").expect("custom script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Clear exec bits so the test also proves permission repair happens
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("clear exec bit");
    }

    Config::ensure_default_scripts_in(&root).expect("default scripts");

    assert_eq!(
        fs::read_to_string(&path).expect("read custom script"),
        "#!/bin/sh\nexit 42\n"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&path)
            .expect("script metadata")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "custom script should be executable");
    }

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn ensure_default_scripts_in_rejects_symlink_without_changing_external_permissions() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let root = test_root("default-script-symlink");
    let _ = fs::remove_dir_all(&root);
    let script = &crate::DEFAULT_SCRIPTS[0];
    let path = root.join(script.relative_path);
    let outside = root.join("outside.sh");
    fs::create_dir_all(path.parent().expect("script parent")).expect("script parent dir");
    fs::write(&outside, "external").expect("outside script");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).expect("outside mode");
    symlink(&outside, &path).expect("script symlink");

    Config::ensure_default_scripts_in(&root).expect_err("script symlink should be rejected");

    assert_eq!(
        fs::read_to_string(&outside).expect("outside contents"),
        "external"
    );
    assert_eq!(
        fs::metadata(&outside)
            .expect("outside metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn enabled_default_script_commands_have_shipped_files() {
    // Stock config should never point at a script that is missing from DEFAULT_SCRIPTS
    let shipped = crate::DEFAULT_SCRIPTS
        .iter()
        .map(|script| script.relative_path)
        .collect::<Vec<_>>();
    let config = Config::default();

    for toggle in config
        .widgets
        .toggles
        .iter()
        .filter(|toggle| toggle.enabled)
    {
        for command in [
            toggle.state_cmd.as_ref(),
            toggle.toggle_cmd.as_ref(),
            toggle.on_cmd.as_ref(),
            toggle.off_cmd.as_ref(),
            toggle.watch_cmd.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let Some(program) = command.program().and_then(std::path::Path::to_str) else {
                continue;
            };
            if program.starts_with("scripts/") {
                assert!(
                    shipped.contains(&program),
                    "default command must be shipped: {program}"
                );
            }
        }
    }
}

#[test]
fn write_default_scripts_in_replaces_user_edited_script_contents() {
    let root = test_root("default-script-overwrite");
    // The explicit write helper is the reset path, so it should replace user edits
    let _ = fs::remove_dir_all(&root);
    let script = crate::DEFAULT_SCRIPTS
        .iter()
        .find(|script| script.relative_path.ends_with("unixnotis-blue-light-off"))
        .expect("blue light off script");
    let path = root.join(script.relative_path);
    fs::create_dir_all(path.parent().expect("script parent")).expect("script parent dir");
    fs::write(&path, "#!/bin/sh\nexit 7\n").expect("custom script");

    Config::write_default_scripts_in(&root).expect("scripts should be reset");

    assert_eq!(
        fs::read_to_string(&path).expect("script contents"),
        script.contents
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Reset scripts must remain directly runnable after atomic replacement
        let mode = fs::metadata(&path)
            .expect("reset script metadata")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "reset script should retain every executable bit"
        );
    }

    let _ = fs::remove_dir_all(root);
}
