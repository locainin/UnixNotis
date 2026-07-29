//! Immutable sender-executable identity checks

use super::super::super::evidence::current_system_identity_matches_sender;
use super::super::*;

#[test]
fn reopened_system_identity_must_remain_protected_and_executable() {
    let (_, trusted) = installed_system_executable();
    let unprotected = FileIdentity {
        uid: 1_000,
        ..trusted
    };
    let non_executable = FileIdentity {
        mode: 0o100_644,
        ..trusted
    };

    assert!(current_system_identity_matches_sender(trusted, trusted));
    assert!(!current_system_identity_matches_sender(
        unprotected,
        trusted
    ));
    assert!(!current_system_identity_matches_sender(
        non_executable,
        trusted
    ));
}
