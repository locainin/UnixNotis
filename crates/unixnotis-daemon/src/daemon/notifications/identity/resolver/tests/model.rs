//! Resolver value-model tests

use super::super::model::CandidateVerification;
use super::*;

#[test]
fn candidate_verification_preserves_mismatch_kind_and_verified_fallback() {
    let record = system_record(
        "org.example.App",
        "Example App",
        "/usr/bin/example-app",
        identity(201, 2_010, 0),
    );
    let mismatch = CandidateVerification {
        record: &record,
        verification: LaunchVerification::DefinitiveMismatch(
            LaunchFailure::ProtectedPayloadMismatch,
        ),
    };
    let verified = CandidateVerification {
        record: &record,
        verification: LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
    };

    assert!(mismatch.is_definitive_mismatch());
    assert_eq!(mismatch.failure(), LaunchFailure::ProtectedPayloadMismatch);
    assert!(!verified.is_definitive_mismatch());
    assert_eq!(verified.failure(), LaunchFailure::DesktopClaimMismatch);
}
