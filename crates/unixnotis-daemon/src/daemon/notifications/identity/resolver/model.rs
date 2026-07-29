//! Internal value types shared by resolver stages

use unixnotis_core::{AttributionDiagnostics, InlineReplyPolicy, NotificationAttribution};

use super::super::desktop_index::{
    DesktopRecord, LaunchFailure, LaunchVerification, VerifiedLaunch,
};

#[derive(Clone, Copy)]
pub(in crate::daemon) struct AppClaim<'claim> {
    pub(in crate::daemon) reported_name: &'claim str,
    pub(in crate::daemon) desktop_entry: Option<&'claim str>,
}

pub(in crate::daemon) struct AttributionResolution {
    pub(in crate::daemon) attribution: NotificationAttribution,
    pub(in crate::daemon) diagnostics: AttributionDiagnostics,
    pub(in crate::daemon) inline_reply_policy: InlineReplyPolicy,
}

#[derive(Clone, Copy)]
pub(super) struct VerifiedDesktopRecord<'record>(
    pub(super) &'record DesktopRecord,
    pub(super) VerifiedLaunch,
);

#[derive(Clone, Copy)]
pub(super) struct CandidateVerification<'record> {
    pub(super) record: &'record DesktopRecord,
    pub(super) verification: LaunchVerification,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum SenderClaimRelation {
    ClaimedApplication,
    DifferentVerifiedApplication,
    DifferentInstalledPackage,
    SamePackageHelper,
    UnknownExecutable,
    TrustedRelay,
}

impl CandidateVerification<'_> {
    pub(super) const fn is_definitive_mismatch(&self) -> bool {
        matches!(self.verification, LaunchVerification::DefinitiveMismatch(_))
    }

    pub(super) const fn failure(&self) -> LaunchFailure {
        match self.verification {
            LaunchVerification::Verified(_) => LaunchFailure::DesktopClaimMismatch,
            LaunchVerification::InsufficientEvidence(reason)
            | LaunchVerification::DefinitiveMismatch(reason) => reason,
        }
    }
}
