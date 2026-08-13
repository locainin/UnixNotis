use super::is_trial_confirmation;

#[test]
fn trial_confirmation_accepts_trimmed_ascii_yes_values() {
    assert!(is_trial_confirmation(" y "));
    assert!(is_trial_confirmation("YES"));
}

#[test]
fn trial_confirmation_rejects_empty_and_unrecognized_values() {
    assert!(!is_trial_confirmation(""));
    assert!(!is_trial_confirmation("true"));
}
