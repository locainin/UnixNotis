//! Conversion from daemon launch evidence into stable diagnostic wire values

use unixnotis_core::{
    AttributionDiagnostics, AttributionStatus, CommandLineQualityView, LaunchAuthorityView,
    LaunchVerificationView, RecordTrust,
};

use super::super::desktop_index::{
    DesktopRecord, LaunchFailure, LaunchVerification, VerifiedLaunch,
};
use super::super::sender::{CommandLineQuality, SenderMetadata};
use super::{AppClaim, AttributionResolution};

pub(super) fn with_diagnostics(
    mut resolution: AttributionResolution,
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    record: Option<&DesktopRecord>,
    verification: LaunchVerification,
) -> AttributionResolution {
    let (verification_view, launch_authority, reason) = match verification {
        LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable) => (
            LaunchVerificationView::Verified,
            LaunchAuthorityView::DedicatedExecutable,
            "verified by dedicated executable identity",
        ),
        LaunchVerification::Verified(VerifiedLaunch::ProtectedPayload) => (
            LaunchVerificationView::Verified,
            LaunchAuthorityView::ProtectedPayload,
            "verified by executable and protected payload identity",
        ),
        LaunchVerification::DefinitiveMismatch(failure)
            if resolution.attribution.status == AttributionStatus::Conflict =>
        {
            (
                LaunchVerificationView::DefinitiveMismatch,
                launch_authority_for_failure(failure),
                launch_failure_label(failure),
            )
        }
        LaunchVerification::InsufficientEvidence(failure)
        | LaunchVerification::DefinitiveMismatch(failure) => (
            LaunchVerificationView::InsufficientEvidence,
            launch_authority_for_failure(failure),
            launch_failure_label(failure),
        ),
    };
    resolution.diagnostics = AttributionDiagnostics {
        claimed_name: claim.reported_name.to_string(),
        claimed_desktop_entry: claim.desktop_entry.unwrap_or_default().to_string(),
        sender_executable: sender.sender_executable.clone().unwrap_or_default(),
        matched_desktop_id: record.map_or_else(String::new, |record| record.id.clone()),
        record_trust: record.map_or(RecordTrust::None, |record| {
            if record.system_origin {
                RecordTrust::System
            } else {
                RecordTrust::User
            }
        }),
        launch_authority,
        command_line_quality: command_line_quality_view(sender.command_line.quality),
        verification: verification_view,
        reason: reason.to_string(),
    };
    resolution
}

pub(super) const fn launch_failure_label(reason: LaunchFailure) -> &'static str {
    match reason {
        LaunchFailure::MissingSenderEvidence => "missing sender process evidence",
        LaunchFailure::MissingCommandLine => "missing command-line evidence",
        LaunchFailure::UnstructuredCommandLine => "unstructured command-line evidence",
        LaunchFailure::EmptyContractNeedsCommandLine => {
            "empty launch contract requires structured command-line evidence"
        }
        LaunchFailure::UnsupportedWrapper => "unsupported launch wrapper",
        LaunchFailure::AmbiguousDesktopAssociation => "ambiguous desktop association",
        LaunchFailure::DynamicOnlyContract => "dynamic-only launch contract",
        LaunchFailure::ExecutableMismatch => "executable identity mismatch",
        LaunchFailure::ProtectedPayloadMismatch => "protected application payload mismatch",
        LaunchFailure::RequiredArgumentMismatch => "required launch argument mismatch",
        LaunchFailure::DesktopClaimMismatch => "desktop claim mismatch",
        LaunchFailure::NoDesktopCandidate => "no desktop application candidate",
    }
}

const fn launch_authority_for_failure(failure: LaunchFailure) -> LaunchAuthorityView {
    match failure {
        LaunchFailure::DynamicOnlyContract => LaunchAuthorityView::DynamicOnly,
        LaunchFailure::AmbiguousDesktopAssociation => LaunchAuthorityView::Ambiguous,
        LaunchFailure::EmptyContractNeedsCommandLine => LaunchAuthorityView::DedicatedExecutable,
        LaunchFailure::ProtectedPayloadMismatch
        | LaunchFailure::MissingCommandLine
        | LaunchFailure::UnstructuredCommandLine => LaunchAuthorityView::ProtectedPayload,
        LaunchFailure::MissingSenderEvidence
        | LaunchFailure::NoDesktopCandidate
        | LaunchFailure::ExecutableMismatch
        | LaunchFailure::RequiredArgumentMismatch
        | LaunchFailure::DesktopClaimMismatch
        | LaunchFailure::UnsupportedWrapper => LaunchAuthorityView::None,
    }
}

const fn command_line_quality_view(quality: CommandLineQuality) -> CommandLineQualityView {
    match quality {
        CommandLineQuality::Structured => CommandLineQualityView::Structured,
        CommandLineQuality::RewrittenProcessTitle => CommandLineQualityView::RewrittenProcessTitle,
        CommandLineQuality::Truncated => CommandLineQualityView::Truncated,
        CommandLineQuality::Unavailable => CommandLineQualityView::Unavailable,
    }
}
