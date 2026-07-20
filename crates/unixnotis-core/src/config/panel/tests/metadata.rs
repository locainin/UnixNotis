use super::super::NotificationMetadataConfig;

#[test]
fn metadata_defaults_keep_existing_runtime_copy() {
    let metadata = NotificationMetadataConfig::default();

    assert_eq!(metadata.critical_label, "ALERT");
    assert_eq!(metadata.relative_minutes, "{value}m");
    assert_eq!(metadata.live_label, "LIVE");
    assert_eq!(metadata.action_count_one, "{count} ACTION");
    assert_eq!(metadata.action_count_many, "{count} ACTIONS");
}

#[test]
fn metadata_text_parses_as_one_nested_panel_block() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        metadata: NotificationMetadataConfig,
    }

    let fixture: Fixture = toml::from_str(
        r#"
        [metadata]
        critical_label = "PRIORITY"
        relative_hours = "{value} hours ago"
        history_label = "ARCHIVE"
        action_count_many = "{count} OPTIONS"
        "#,
    )
    .expect("metadata block should parse");

    assert_eq!(fixture.metadata.critical_label, "PRIORITY");
    assert_eq!(fixture.metadata.relative_hours, "{value} hours ago");
    assert_eq!(fixture.metadata.history_label, "ARCHIVE");
    assert_eq!(fixture.metadata.action_count_many, "{count} OPTIONS");
    assert_eq!(fixture.metadata.low_label, "LOW");
}
