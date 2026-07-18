use super::super::runner::post_import_css_check_with;
use super::*;
use crate::test_support::{test_env_lock, EnvGuard};

#[test]
fn import_runner_reports_a_missing_bundle() {
    let missing = std::env::temp_dir().join("unixnotis-missing-runner-bundle.unixnotis");
    let _ = fs::remove_file(&missing);

    let error = crate::preset::import::run_import(&missing, &[], true, false, false)
        .expect_err("missing bundle should fail");

    assert!(
        error.to_string().contains("does not exist")
            || error.to_string().contains("read preset bundle")
    );
}

#[test]
fn post_import_css_check_targets_imported_config() {
    let _lock = test_env_lock();
    let imported = TempDirGuard::new("post-import-valid");
    let redirected = TempDirGuard::new("post-import-invalid-override");
    redirected.write("config.toml", "this is not toml");
    let _override_path = EnvGuard::set(
        "UNIXNOTIS_CONFIG_PATH",
        redirected.path.join("config.toml").as_os_str(),
    );

    post_import_css_check_with(&imported.path, |requested_path| {
        assert_eq!(requested_path, Some(imported.path.join("config.toml")));
        Ok(())
    })
    .expect("validate the imported configuration");
}

#[test]
fn environment_override_cannot_redirect_post_import_validation() {
    let _lock = test_env_lock();
    let imported = TempDirGuard::new("post-import-invalid");
    let redirected = TempDirGuard::new("post-import-valid-override");
    redirected.write("config.toml", "");
    let _override_path = EnvGuard::set(
        "UNIXNOTIS_CONFIG_PATH",
        redirected.path.join("config.toml").as_os_str(),
    );

    post_import_css_check_with(&imported.path, |requested_path| {
        assert_eq!(requested_path, Some(imported.path.join("config.toml")));
        anyhow::bail!("imported config rejected")
    })
    .expect_err("the imported malformed configuration must be rejected");
}
