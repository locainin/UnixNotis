use super::*;

#[test]
fn parse_stat_payload_accepts_v1() {
    let payload = br#"{"api_version":1,"text":" 42% "}"#;
    let parsed = parse_stat_plugin_payload(
        payload,
        PluginOutputLimits {
            max_output_bytes: 1024,
        },
    )
    .expect("payload should parse");
    assert_eq!(parsed.text, "42%");
}

#[test]
fn parse_stat_payload_rejects_wrong_version() {
    let payload = br#"{"api_version":2,"text":"42%"}"#;
    let err = parse_stat_plugin_payload(
        payload,
        PluginOutputLimits {
            max_output_bytes: 1024,
        },
    )
    .expect_err("version should be rejected");
    assert!(err.contains("unsupported plugin api_version"));
}

#[test]
fn parse_card_payload_accepts_title_override() {
    let payload = br#"{"api_version":1,"title":"Weather","text":"72F, clear"}"#;
    let parsed = parse_card_plugin_payload(
        payload,
        PluginOutputLimits {
            max_output_bytes: 1024,
        },
    )
    .expect("payload should parse");
    assert_eq!(parsed.title.as_deref(), Some("Weather"));
    assert_eq!(parsed.text, "72F, clear");
}

#[test]
fn parse_card_payload_rejects_oversized_payload() {
    let payload = br#"{"api_version":1,"text":"ok"}"#;
    let err = parse_card_plugin_payload(
        payload,
        PluginOutputLimits {
            max_output_bytes: 8,
        },
    )
    .expect_err("payload should exceed limit");
    assert!(err.contains("max_output_bytes"));
}
