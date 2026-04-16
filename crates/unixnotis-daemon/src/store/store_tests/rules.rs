use super::*;

#[test]
fn contains_ci_matches_ascii() {
    assert!(contains_ci("Signal-Desktop", "signal"));
    assert!(contains_ci("signal-desktop", "Signal"));
    assert!(!contains_ci("signal-desktop", "brave"));
    assert!(contains_ci("mixedCase", "case"));
    assert!(contains_ci("mixedCase", ""));
}
