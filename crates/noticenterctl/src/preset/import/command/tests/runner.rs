use super::*;

#[test]
fn import_runner_reports_a_missing_bundle() {
    let missing = std::env::temp_dir().join("unixnotis-missing-runner-bundle.unixnotis");
    let _ = fs::remove_file(&missing);

    let error = crate::preset::import::run_import(&missing, &[], true, false)
        .expect_err("missing bundle should fail");

    assert!(
        error.to_string().contains("does not exist")
            || error.to_string().contains("read preset bundle")
    );
}
