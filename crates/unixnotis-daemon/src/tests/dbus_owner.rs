use super::{owner_name_is_self, owner_state_matches};

#[test]
fn owner_state_matches_expected_presence_and_release() {
    // Non-empty owner names mean the bus name is currently owned
    assert!(owner_state_matches(Some(":1.42"), true));
    assert!(!owner_state_matches(Some(":1.42"), false));

    // Empty owner names mean the bus name was released
    assert!(owner_state_matches(Some(""), false));
    assert!(!owner_state_matches(Some(""), true));

    // Missing signal data is treated like released ownership
    assert!(owner_state_matches(None, false));
}

#[test]
fn owner_name_is_self_requires_exact_unique_name_match() {
    // D-Bus unique names are exact tokens, so prefix or suffix matches must not pass
    assert!(owner_name_is_self(Some(":1.7"), ":1.7"));
    assert!(!owner_name_is_self(Some(":1.70"), ":1.7"));
    assert!(!owner_name_is_self(None, ":1.7"));
}
