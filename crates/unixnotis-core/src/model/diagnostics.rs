//! Structured application-attribution evidence for diagnostic clients

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

/// Trust level of the desktop record selected by launch verification
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u8)]
pub enum RecordTrust {
    #[default]
    None = 0,
    Portal = 1,
    System = 2,
    User = 3,
}

/// Evidence that establishes or weakens one desktop launch association
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u8)]
pub enum LaunchAuthorityView {
    #[default]
    None = 0,
    DedicatedExecutable = 1,
    ProtectedPayload = 2,
    DynamicOnly = 3,
    Ambiguous = 4,
}

/// Reliability of the argument boundaries read for the sender process
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u8)]
pub enum CommandLineQualityView {
    Structured = 0,
    RewrittenProcessTitle = 1,
    Truncated = 2,
    #[default]
    Unavailable = 3,
}

/// Summary of the strongest launch-verification result
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u8)]
pub enum LaunchVerificationView {
    Verified = 0,
    #[default]
    InsufficientEvidence = 1,
    DefinitiveMismatch = 2,
}

/// Bounded evidence retained for notification explanation and debug logs
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AttributionDiagnostics {
    pub claimed_name: String,
    pub claimed_desktop_entry: String,
    pub sender_executable: String,
    pub matched_desktop_id: String,
    pub record_trust: RecordTrust,
    pub launch_authority: LaunchAuthorityView,
    pub command_line_quality: CommandLineQualityView,
    pub verification: LaunchVerificationView,
    pub reason: String,
}

#[cfg(test)]
#[path = "tests/diagnostics.rs"]
mod tests;
