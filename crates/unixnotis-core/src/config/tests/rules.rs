use super::{RuleConfig, RuleUrgency};

#[test]
fn rule_urgency_numeric_values_round_trip_to_spec_numbers() {
    // The config layer stores urgency as the freedesktop numeric values
    assert_eq!(RuleUrgency::Low.as_u8(), 0);
    assert_eq!(RuleUrgency::Normal.as_u8(), 1);
    assert_eq!(RuleUrgency::Critical.as_u8(), 2);

    let serialized = toml::to_string(&RuleConfig {
        urgency: Some(RuleUrgency::Critical),
        ..RuleConfig::default()
    })
    .expect("rule config should serialize");

    assert!(serialized.contains("urgency = 2"));
}

#[test]
fn rule_urgency_accepts_numeric_and_string_aliases() {
    for (value, expected) in [
        ("0", RuleUrgency::Low),
        ("1", RuleUrgency::Normal),
        ("2", RuleUrgency::Critical),
        ("\"low\"", RuleUrgency::Low),
        ("\"normal\"", RuleUrgency::Normal),
        ("\"critical\"", RuleUrgency::Critical),
        ("\" LOW \"", RuleUrgency::Low),
    ] {
        let config: RuleConfig =
            toml::from_str(&format!("urgency = {value}")).expect("urgency alias should parse");
        assert_eq!(
            config.urgency,
            Some(expected),
            "alias should parse: {value}"
        );
    }
}

#[test]
fn rule_urgency_rejects_out_of_range_numeric_values() {
    for value in ["-1", "3", "256"] {
        let err = toml::from_str::<RuleConfig>(&format!("urgency = {value}"))
            .expect_err("out of range urgency should fail");
        assert!(
            err.to_string().contains("urgency must be 0"),
            "error should explain valid urgency values: {err}"
        );
    }
}

#[test]
fn rule_urgency_rejects_unknown_string_aliases() {
    let err = toml::from_str::<RuleConfig>("urgency = \"urgent\"")
        .expect_err("unknown urgency alias should fail");

    assert!(
        err.to_string().contains("urgency must be 0"),
        "error should explain valid urgency values: {err}"
    );
}

#[test]
fn rule_urgency_reports_expected_type_for_wrong_shape() {
    let err = toml::from_str::<RuleConfig>("urgency = []")
        .expect_err("array urgency should fail with expected type");

    assert!(
        err.to_string().contains("an urgency value"),
        "error should include the visitor expectation: {err}"
    );
}

#[test]
fn rule_urgency_accepts_upper_valid_signed_integer() {
    let config: RuleConfig = toml::from_str("urgency = 2").expect("critical should parse");

    assert_eq!(config.urgency, Some(RuleUrgency::Critical));
}

#[test]
fn rule_force_urgency_uses_same_parser_as_match_urgency() {
    let config: RuleConfig =
        toml::from_str("force_urgency = \"critical\"").expect("force urgency should parse");

    assert_eq!(config.force_urgency, Some(RuleUrgency::Critical));
}
