//! Structured attribution construction and trust-domain grouping

use unixnotis_core::{
    AttributionDiagnostics, AttributionReason, AttributionStatus, IdentityAssurance,
    InteractionPolicies, NotificationAttribution,
};

use super::super::desktop_index::{
    normalize_name, DesktopIdentityIndex, DesktopRecord, LaunchFailure, LaunchVerification,
    VerifiedLaunch,
};
use super::super::policy::inline_reply_policy;
use super::super::sender::SenderMetadata;
use super::diagnostics::{launch_failure_label, with_diagnostics};
use super::model::VerifiedDesktopRecord;
use super::{AppClaim, AttributionResolution};

pub(in crate::daemon) fn unknown_reply_denied(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    reason: &str,
) -> AttributionResolution {
    let detail = sender.sender_executable.as_deref().map_or_else(
        || reason.to_string(),
        |path| format!("{reason}; source {path}"),
    );
    let resolution = policy_resolution(NotificationAttribution::unresolved(
        claim.reported_name,
        AttributionReason::MissingSenderEvidence,
        &detail,
        sender_claim_group_key(AttributionStatus::Unresolved, claim.reported_name, sender),
    ));
    with_diagnostics(
        resolution,
        claim,
        sender,
        None,
        LaunchVerification::InsufficientEvidence(LaunchFailure::MissingSenderEvidence),
    )
}

pub(super) fn resolution_for_portal_record(
    record: &DesktopRecord,
    reported_name: &str,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    let portal = trusted_portal_path(sender, index).map_or_else(
        || "desktop portal".to_string(),
        |path| path.display().to_string(),
    );
    let canonical = index.canonical_record_for_record(record);
    let canonical_id = index.canonical_id_for_record(record);
    let attribution = NotificationAttribution::associated(
        &canonical.display_name,
        reported_name,
        canonical_id,
        &canonical.badge_icon,
        IdentityAssurance::PortalAssociated,
        InteractionPolicies::CONFIRM_ACTIONS,
        AttributionReason::PortalAppIdAssociation,
        &format!("Mediated by {portal}"),
        format!("associated:portal-app:{canonical_id}"),
    );
    policy_resolution(attribution)
}

pub(super) fn trusted_portal_path<'index>(
    sender: &SenderMetadata,
    index: &'index DesktopIdentityIndex,
) -> Option<&'index std::path::Path> {
    let identity = sender.sender_executable_identity?;
    let path = std::path::Path::new(sender.sender_executable.as_deref()?);
    index.trusted_portal_path(identity, path)
}

pub(super) fn resolution_for_record(
    verified: VerifiedDesktopRecord<'_>,
    reported_name: &str,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    let record = verified.0;
    if !reported_name.trim().is_empty() && !index.record_matches_claim(record, reported_name) {
        return conflict_resolution(
            reported_name,
            sender,
            record,
            index,
            LaunchFailure::DesktopClaimMismatch,
        );
    }
    let canonical = index.canonical_record_for_record(record);
    let canonical_id = index.canonical_id_for_record(record);
    let source = record
        .runtime_executable_path
        .as_deref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    if record.system_association {
        let reason = match verified.1 {
            VerifiedLaunch::DedicatedExecutable | VerifiedLaunch::PackageLauncherTarget => {
                AttributionReason::ExactSystemExecutable
            }
            VerifiedLaunch::ProtectedPayload => AttributionReason::ProtectedPayloadMatch,
        };
        return policy_resolution(NotificationAttribution::associated(
            &canonical.display_name,
            reported_name,
            canonical_id,
            &canonical.badge_icon,
            IdentityAssurance::SystemAssociated,
            InteractionPolicies::NATIVE_COMPATIBILITY,
            reason,
            &source,
            format!("associated:system-app:{canonical_id}"),
        ));
    }

    let origin = record.desktop_identity.map_or_else(
        || "unknown".to_string(),
        super::super::executable::FileIdentity::group_fragment,
    );
    policy_resolution(NotificationAttribution::associated(
        &canonical.display_name,
        reported_name,
        canonical_id,
        &canonical.badge_icon,
        IdentityAssurance::UserAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        AttributionReason::ExactUserExecutable,
        &source,
        format!(
            "recognized:user-app:{origin}:{canonical_id}:{}",
            sender_identity_fragment(sender)
        ),
    ))
}

pub(super) fn recognized_resolution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    failure: LaunchFailure,
    detail: &str,
) -> AttributionResolution {
    let canonical = index.canonical_record_for_record(record);
    let canonical_id = index.canonical_id_for_record(record);
    let source = sender.sender_executable.as_deref().map_or_else(
        || detail.to_string(),
        |path| format!("{detail}; source {path}"),
    );
    let group_key = if record.system_origin {
        format!(
            "recognized:system-app:{canonical_id}:{}",
            sender_identity_fragment(sender)
        )
    } else {
        let origin = record.desktop_identity.map_or_else(
            || "unknown".to_string(),
            super::super::executable::FileIdentity::group_fragment,
        );
        format!(
            "recognized:user-app:{origin}:{canonical_id}:{}",
            sender_identity_fragment(sender)
        )
    };
    let assurance = if record.system_origin {
        IdentityAssurance::SystemAssociated
    } else {
        IdentityAssurance::UserAssociated
    };
    policy_resolution(NotificationAttribution::associated(
        &canonical.display_name,
        claim.reported_name,
        canonical_id,
        &canonical.badge_icon,
        assurance,
        InteractionPolicies::DENY,
        attribution_reason_for_failure(failure),
        &source,
        group_key,
    ))
}

pub(super) fn conflict_from_candidate(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    record: &DesktopRecord,
    failure: LaunchFailure,
) -> AttributionResolution {
    with_diagnostics(
        conflict_resolution(claim.reported_name, sender, record, index, failure),
        claim,
        sender,
        Some(record),
        LaunchVerification::DefinitiveMismatch(failure),
    )
}

fn conflict_resolution(
    reported_name: &str,
    sender: &SenderMetadata,
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    failure: LaunchFailure,
) -> AttributionResolution {
    let label = launch_failure_label(failure);
    let detail = sender.sender_executable.as_deref().map_or_else(
        || label.to_string(),
        |path| format!("{label}; source {path}"),
    );
    let desktop_id = index.canonical_id_for_record(record);
    policy_resolution(NotificationAttribution::conflict(
        reported_name,
        desktop_id,
        attribution_reason_for_failure(failure),
        &detail,
        sender_claim_group_key(AttributionStatus::Conflict, reported_name, sender),
    ))
}

pub(super) fn policy_resolution(attribution: NotificationAttribution) -> AttributionResolution {
    AttributionResolution {
        inline_reply_policy: inline_reply_policy(attribution.interactions),
        attribution,
        diagnostics: AttributionDiagnostics::default(),
    }
}

const fn attribution_reason_for_failure(failure: LaunchFailure) -> AttributionReason {
    match failure {
        LaunchFailure::MissingSenderEvidence => AttributionReason::MissingSenderEvidence,
        LaunchFailure::MissingCommandLine
        | LaunchFailure::UnstructuredCommandLine
        | LaunchFailure::EmptyContractNeedsCommandLine => AttributionReason::MissingCommandLine,
        LaunchFailure::UnsupportedWrapper | LaunchFailure::LauncherBindingChanged => {
            AttributionReason::UnsupportedWrapper
        }
        LaunchFailure::AmbiguousDesktopAssociation | LaunchFailure::RequiredArgumentMismatch => {
            AttributionReason::AmbiguousDesktopRecords
        }
        LaunchFailure::DynamicOnlyContract => AttributionReason::DynamicLaunchContract,
        LaunchFailure::NoDesktopCandidate => AttributionReason::NoDesktopCandidate,
        LaunchFailure::ExecutableMismatch => AttributionReason::ExecutableMismatch,
        LaunchFailure::ProtectedPayloadMismatch => AttributionReason::ProtectedPayloadMismatch,
        LaunchFailure::DesktopClaimMismatch => AttributionReason::ApplicationClaimMismatch,
    }
}

pub(super) fn sender_claim_group_key(
    status: AttributionStatus,
    reported_name: &str,
    sender: &SenderMetadata,
) -> String {
    let claim = normalize_name(reported_name);
    let prefix = match status {
        AttributionStatus::Unresolved => "unresolved",
        AttributionStatus::Conflict => "conflict",
        AttributionStatus::Verified | AttributionStatus::Recognized | AttributionStatus::Relay => {
            "unknown"
        }
    };
    format!("{prefix}:{}:{claim}", sender_identity_fragment(sender))
}

fn sender_identity_fragment(sender: &SenderMetadata) -> String {
    sender.sender_executable_identity.map_or_else(
        || "missing".to_string(),
        super::super::executable::FileIdentity::group_fragment,
    )
}
