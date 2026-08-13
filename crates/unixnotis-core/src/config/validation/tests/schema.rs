use super::*;

fn deserialize_config(contents: &str) -> Result<(Config, Vec<String>), String> {
    deserialize_current_config(contents)
}

#[test]
fn current_schema_parses_with_current_defaults() {
    let input = format!("config_version = {CURRENT_CONFIG_VERSION}\n[media]\n");
    let (config, ignored) = deserialize_config(&input).expect("parse current config");

    assert!(ignored.is_empty());
    assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
    assert_eq!(
        config.media.local_art_policy,
        crate::MediaLocalArtPolicy::AllAdmitted
    );
}

#[test]
fn current_schema_preserves_explicit_values() {
    let input = format!(
        "config_version = {CURRENT_CONFIG_VERSION}\n[panel]\nwidth = 517\n[media]\nlocal_art_policy = \"exact_executable_only\"\nlocal_art_executable_allowlist = [\"/usr/bin/player\"]\n"
    );
    let (config, ignored) = deserialize_config(&input).expect("parse explicit current config");

    assert!(ignored.is_empty());
    assert_eq!(config.panel.width, 517);
    assert_eq!(
        config.media.local_art_policy,
        crate::MediaLocalArtPolicy::ExactExecutableOnly
    );
    assert_eq!(
        config.media.local_art_executable_allowlist,
        ["/usr/bin/player"]
    );
}

#[test]
fn every_pre_v5_schema_is_rejected_without_migration() {
    for version in 0..CURRENT_CONFIG_VERSION {
        let input = if version == 0 {
            String::new()
        } else {
            format!("config_version = {version}\n")
        };
        let error = deserialize_config(&input).expect_err("reject pre-v5 config");
        assert_eq!(error, format!("unsupported config version {version}"));
    }
}

#[test]
fn future_schema_is_rejected_without_guessing() {
    let error = deserialize_config("config_version = 999\n").expect_err("reject future config");

    assert_eq!(error, "unsupported config version 999");
}

#[test]
fn oversized_schema_version_is_rejected_without_integer_wrapping() {
    let error = deserialize_config("config_version = 4294967296\n")
        .expect_err("reject schema version larger than u32");

    assert_eq!(error, "unsupported config version 4294967296");
}

#[test]
fn negative_or_non_integer_schema_versions_are_rejected() {
    for input in [
        "config_version = -1\n",
        "config_version = \"5\"\n",
        "config_version = true\n",
    ] {
        let error = deserialize_config(input).expect_err("reject malformed schema version");
        assert_eq!(error, "config_version must be a non-negative integer");
    }
}

#[test]
fn current_schema_reports_unknown_keys_without_rejecting_valid_fields() {
    let input = format!(
        "config_version = {CURRENT_CONFIG_VERSION}\n[panel]\nwidth = 500\nunknown_panel_key = true\n"
    );
    let (config, ignored) = deserialize_config(&input).expect("parse current config");

    assert_eq!(config.panel.width, 500);
    assert_eq!(ignored, ["panel.unknown_panel_key"]);
}

#[test]
fn non_table_configuration_root_is_rejected() {
    let error = deserialize_config("[1, 2, 3]").expect_err("reject non-table TOML root");

    assert!(!error.is_empty());
}
