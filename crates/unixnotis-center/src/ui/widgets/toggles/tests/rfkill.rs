use super::parse_rfkill_state;

#[test]
fn rfkill_state_is_active_only_when_every_discovered_device_is_soft_blocked() {
    let output = br#"{
        "rfkilldevices": [
            {"id": 0, "type": "wlan", "soft": "blocked", "hard": "unblocked"},
            {"id": 1, "type": "bluetooth", "soft": "blocked", "hard": "blocked"}
        ]
    }"#;

    let state = parse_rfkill_state(output).expect("parse blocked rfkill state");

    assert_eq!(state.device_count, 2);
    assert!(state.all_soft_blocked);
    assert!(state.is_airplane_mode_active());
}

#[test]
fn rfkill_state_is_inactive_when_one_device_is_unblocked() {
    let output = br#"{
        "rfkilldevices": [
            {"soft": "blocked"},
            {"soft": "unblocked"}
        ]
    }"#;

    let state = parse_rfkill_state(output).expect("parse mixed rfkill state");

    assert!(!state.all_soft_blocked);
    assert!(!state.is_airplane_mode_active());
}

#[test]
fn rfkill_state_is_inactive_when_no_devices_exist() {
    let state = parse_rfkill_state(br#"{"rfkilldevices": []}"#).expect("parse empty state");

    assert_eq!(state.device_count, 0);
    assert!(!state.is_airplane_mode_active());
}

#[test]
fn malformed_rfkill_json_is_rejected() {
    assert!(parse_rfkill_state(br#"{"rfkilldevices": [}"#).is_err());
}
