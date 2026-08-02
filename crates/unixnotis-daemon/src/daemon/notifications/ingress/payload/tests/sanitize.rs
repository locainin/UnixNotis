use super::*;
#[test]
fn parse_actions_caps_pairs() {
    let mut raw = Vec::new();
    for idx in 0..(MAX_ACTIONS + 10) {
        raw.push(format!("key-{idx}"));
        raw.push(format!("label-{idx}"));
    }

    let actions = parse_actions(raw);
    assert_eq!(actions.len(), MAX_ACTIONS);
}

#[test]
fn parse_actions_ignores_dangling_key_without_label() {
    let actions = parse_actions(vec![
        "default".to_string(),
        "Open".to_string(),
        "orphan-key".to_string(),
    ]);

    // D-Bus action arrays are pairs; a trailing key cannot produce a safe button
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].key, "default");
    assert_eq!(actions[0].label, "Open");
}

#[test]
fn parse_actions_reserves_capacity_for_complete_pairs_only() {
    let actions = parse_actions(vec![
        "default".to_string(),
        "Open".to_string(),
        "dismiss".to_string(),
        "Dismiss".to_string(),
    ]);

    assert_eq!(actions.len(), 2);
    assert_eq!(actions.capacity(), 2);
}

#[test]
fn sanitize_hints_drops_untrusted_and_bounds_strings() {
    let mut hints = HashMap::<String, OwnedValue>::new();
    hints.insert("transient".to_string(), OwnedValue::from(true));
    hints.insert("urgency".to_string(), OwnedValue::from(9u32));
    hints.insert(
        "sound-name".to_string(),
        string_to_owned_value(&"n".repeat(5000)).expect("sound-name"),
    );
    hints.insert("image-data".to_string(), OwnedValue::from(123u32));
    hints.insert(
        "x-custom".to_string(),
        string_to_owned_value("custom").expect("custom"),
    );

    let sanitized = sanitize_hints_for_storage(hints);
    assert_eq!(sanitized.len(), 3);
    assert!(sanitized.contains_key("transient"));
    assert!(sanitized.contains_key("sound-name"));
    assert_eq!(
        u32::try_from(sanitized.get("urgency").expect("urgency")),
        Ok(2)
    );

    let sound_name = owned_to_string(
        sanitized
            .get("sound-name")
            .expect("sound-name should remain"),
    )
    .expect("sound-name should be string");
    assert!(sound_name.len() <= 2048);
}

#[test]
fn parse_urgency_hint_accepts_byte_and_integer_values_with_cap() {
    assert_eq!(parse_urgency_hint(&OwnedValue::from(0u8)), Some(0));
    assert_eq!(parse_urgency_hint(&OwnedValue::from(1u32)), Some(1));
    assert_eq!(parse_urgency_hint(&OwnedValue::from(99u32)), Some(2));
    assert_eq!(
        parse_urgency_hint(&string_to_owned_value("high").expect("string")),
        None
    );
}

#[test]
fn owned_to_string_accepts_only_string_values() {
    assert_eq!(
        owned_to_string(&string_to_owned_value("sound").expect("string")).as_deref(),
        Some("sound")
    );
    assert_eq!(owned_to_string(&OwnedValue::from(7u32)), None);
}
