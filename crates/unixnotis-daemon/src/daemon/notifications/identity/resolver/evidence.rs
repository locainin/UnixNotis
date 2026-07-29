//! Sender, lineage, and contradiction evidence evaluation

use super::super::desktop_index::{
    verify_record_launch, DesktopIdentityIndex, DesktopRecord, LaunchFailure, LaunchVerification,
    VerifiedLaunch,
};
use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::super::sender::{CommandLineEvidence, SenderMetadata};
use super::model::{CandidateVerification, SenderClaimRelation};

pub(super) fn lineage_association<'record>(
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    results: &[&CandidateVerification<'record>],
) -> Option<(&'record DesktopRecord, String)> {
    for ancestor in &sender.ancestors {
        for result in results {
            let record = result.record;
            if !record.system_association
                || !record
                    .executable_identity
                    .is_some_and(|identity| identity.same_file(ancestor.executable_identity))
            {
                continue;
            }
            let verification = verify_ancestor_record(record, index, ancestor.executable_identity);
            if matches!(
                verification,
                LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable)
            ) {
                return Some((
                    record,
                    format!(
                        "Same-user ancestor {} matched the application executable",
                        ancestor.executable
                    ),
                ));
            }
        }
    }
    None
}

fn verify_ancestor_record(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    identity: FileIdentity,
) -> LaunchVerification {
    let Some(path) = record.executable_path.as_deref() else {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::MissingSenderEvidence);
    };
    let Some(current) = executable_evidence_for_path(path) else {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::MissingSenderEvidence);
    };
    if !current_system_identity_matches_sender(current.identity, identity) {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
    }
    verify_record_launch(record, index, identity, &CommandLineEvidence::default())
}

pub(super) fn verify_record_sender(
    record: &DesktopRecord,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> LaunchVerification {
    if !record.association_eligible {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper);
    }
    let Some(record_identity) = record.executable_identity else {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper);
    };
    let Some(sender_identity) = sender.sender_executable_identity else {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::MissingSenderEvidence);
    };
    if !record_identity.same_file(sender_identity) {
        return LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch);
    }

    if record.system_association {
        if !sender_identity.is_system_managed() || !sender_identity.is_executable_regular() {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        }
        let Some(path) = record.executable_path.as_deref() else {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        };
        let Some(current) = executable_evidence_for_path(path) else {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        };
        if !current_system_identity_matches_sender(current.identity, sender_identity) {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        }
        return verify_record_launch(record, index, sender_identity, &sender.command_line);
    }

    // Exact user-local executable identity is recognition evidence without action authority
    LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable)
}

pub(super) fn candidate_proves_conflict(
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    candidate: &CandidateVerification<'_>,
) -> bool {
    match candidate.failure() {
        // A structured protected payload or verified application claim is direct evidence
        LaunchFailure::ProtectedPayloadMismatch | LaunchFailure::DesktopClaimMismatch => true,
        // Executable inequality matters only after another immutable owner is established
        LaunchFailure::ExecutableMismatch => matches!(
            sender_claim_relation(sender, index, candidate.record),
            SenderClaimRelation::DifferentVerifiedApplication
        ),
        LaunchFailure::MissingSenderEvidence
        | LaunchFailure::MissingCommandLine
        | LaunchFailure::UnstructuredCommandLine
        | LaunchFailure::EmptyContractNeedsCommandLine
        | LaunchFailure::UnsupportedWrapper
        | LaunchFailure::AmbiguousDesktopAssociation
        | LaunchFailure::DynamicOnlyContract
        | LaunchFailure::RequiredArgumentMismatch
        | LaunchFailure::NoDesktopCandidate => false,
    }
}

pub(super) fn sender_claim_relation(
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    claimed_record: &DesktopRecord,
) -> SenderClaimRelation {
    let Some(sender_identity) = sender.sender_executable_identity else {
        return SenderClaimRelation::UnknownExecutable;
    };
    if index.trusted_relay_path(sender_identity).is_some() {
        return SenderClaimRelation::TrustedRelay;
    }
    if claimed_record
        .executable_identity
        .is_some_and(|identity| identity.same_file(sender_identity))
    {
        return SenderClaimRelation::ClaimedApplication;
    }
    if index
        .records_for_executable(sender_identity)
        .into_iter()
        .any(|record| record.system_association)
    {
        // Exact same-family executable identity returned above before this lookup
        return SenderClaimRelation::DifferentVerifiedApplication;
    }

    if sender
        .install_provenance
        .same_application_source(&claimed_record.executable_provenance)
    {
        return SenderClaimRelation::SamePackageHelper;
    }
    if sender.install_provenance.is_known() && claimed_record.executable_provenance.is_known() {
        // Package inequality rules out a same-package helper but does not identify another app
        return SenderClaimRelation::DifferentInstalledPackage;
    }
    SenderClaimRelation::UnknownExecutable
}

pub(super) const fn current_system_identity_matches_sender(
    current: FileIdentity,
    sender_identity: FileIdentity,
) -> bool {
    current.same_file(sender_identity)
        && current.is_system_managed()
        && current.is_executable_regular()
}
