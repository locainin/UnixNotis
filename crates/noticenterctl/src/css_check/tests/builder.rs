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

#[test]
fn missing_active_css_file_remains_an_error_inside_the_completed_report() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-css-report-missing-layer-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create CSS report root");
    let config_path = root.join("config.toml");
    let config = Config::default();
    std::fs::write(
        &config_path,
        format!("config_version = {}\n", config.config_version),
    )
    .expect("write CSS report config");
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("resolve CSS report theme paths");
    for path in [
        paths.base_css,
        paths.popup_css,
        paths.widgets_css,
        paths.media_css,
    ] {
        std::fs::write(path, "/* readable CSS layer */").expect("write readable CSS layer");
    }

    let report = build_report_with_parser(&config_path, &config, |_files, _root, _display| {
        Ok((Vec::new(), 0))
    })
    .expect("per-file failures belong in the completed report");

    assert_eq!(report.error_count(), 1);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message == "file not found"));
    std::fs::remove_dir_all(root).expect("remove CSS report root");
}
