use super::*;

#[test]
fn report_builder_rejects_a_missing_config_directory() {
    let config_path = std::env::temp_dir()
        .join(format!(
            "unixnotis-css-report-missing-{}",
            std::process::id()
        ))
        .join("config.toml");

    let error = build_report(&config_path, &Config::default())
        .expect_err("a missing config directory must be rejected");

    assert!(error.to_string().contains("config directory not found"));
}
