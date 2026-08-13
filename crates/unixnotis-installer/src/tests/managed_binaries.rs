use super::validate_managed_binary_names;

#[test]
fn managed_binary_names_preserve_supported_order_and_remove_duplicates() {
    let names = vec![
        "unixnotis-center".to_string(),
        "noticenterctl".to_string(),
        "unixnotis-center".to_string(),
    ];

    let validated = validate_managed_binary_names(names).expect("supported managed names");

    assert_eq!(
        validated,
        vec!["unixnotis-center".to_string(), "noticenterctl".to_string()]
    );
}

#[test]
fn managed_binary_names_accept_the_complete_runtime_set() {
    let names = [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "unixnotis-svg-renderer",
        "unixnotis-css-validate",
        "noticenterctl",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    assert_eq!(
        validate_managed_binary_names(names.clone()).expect("supported runtime names"),
        names
    );
}

#[test]
fn managed_binary_names_reject_path_components_before_allowlist_lookup() {
    for name in [
        "../../.bashrc",
        "/tmp/victim",
        "bin/unixnotis-daemon",
        ".",
        "",
    ] {
        let error = validate_managed_binary_names(vec![name.to_string()])
            .expect_err("path-shaped managed name must fail");

        assert!(error.to_string().contains("invalid path"), "{name:?}");
    }
}

#[test]
fn managed_binary_names_reject_unknown_normal_components() {
    let error = validate_managed_binary_names(vec!["unixnotis-unknown".to_string()])
        .expect_err("unknown managed name must fail");

    assert!(error.to_string().contains("unsupported name"));
}
