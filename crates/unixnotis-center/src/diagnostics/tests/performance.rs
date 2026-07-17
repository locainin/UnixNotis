use super::super::performance::enabled_value;

#[test]
fn documented_probe_switches_enable_collection() {
    for value in ["1", "true", "TRUE", "yes", "YES", "on", "ON"] {
        assert!(
            enabled_value(Some(value)),
            "value should enable probe: {value}"
        );
    }
}

#[test]
fn missing_or_unknown_probe_switches_keep_collection_disabled() {
    for value in [None, Some(""), Some("0"), Some("false"), Some("random")] {
        assert!(
            !enabled_value(value),
            "value should not enable probe: {value:?}"
        );
    }
}
