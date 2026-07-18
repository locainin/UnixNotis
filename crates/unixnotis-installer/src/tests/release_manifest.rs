use super::validate_release_binary_names;

#[test]
fn release_binary_names_preserve_supported_order_and_remove_duplicates() {
    let names = vec![
        "unixnotis-center".to_string(),
        "noticenterctl".to_string(),
        "unixnotis-center".to_string(),
    ];

    let validated = validate_release_binary_names(names).expect("supported release names");

    assert_eq!(
        validated,
        vec!["unixnotis-center".to_string(), "noticenterctl".to_string()]
    );
}

#[test]
fn release_binary_names_accept_the_complete_supported_runtime_set() {
    let names = [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "unixnotis-css-validate",
        "noticenterctl",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    assert_eq!(
        validate_release_binary_names(names.clone()).expect("supported runtime names"),
        names
    );
}

#[test]
fn release_binary_names_reject_path_components_before_allowlist_lookup() {
    for name in ["../unixnotis-daemon", "bin/unixnotis-daemon", ".", ""] {
        let error = validate_release_binary_names(vec![name.to_string()])
            .expect_err("path-shaped release name must fail");

        assert!(
            error.to_string().contains("invalid binary path"),
            "{name:?}"
        );
    }
}

#[test]
fn release_binary_names_reject_unknown_normal_components() {
    let error = validate_release_binary_names(vec!["unixnotis-future-tool".to_string()])
        .expect_err("unknown release name must fail");

    assert!(error.to_string().contains("unsupported binary name"));
}
