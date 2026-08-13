//! Evidence-based desktop launch verification

mod authority;
mod contract;
mod payload;

#[cfg(test)]
mod tests;

use super::super::executable::FileIdentity;
use super::super::sender::CommandLineEvidence;
use super::launcher::launcher_binding_is_current;
use super::model::{
    DesktopIdentityIndex, DesktopRecord, LaunchAuthority, LaunchFailure, LaunchVerification,
};
use authority::classify_launch_authority;
use contract::verify_dedicated;
use payload::{literal_file_identities_are_current, verify_protected_payload};

pub(super) const MAX_PROCESS_ARGUMENTS: usize = 256;

pub(super) fn verify_record_launch(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    sender_identity: FileIdentity,
    command_line: &CommandLineEvidence,
) -> LaunchVerification {
    verify_record_launch_with(
        record,
        index,
        sender_identity,
        command_line,
        launcher_binding_is_current,
    )
}

pub(super) fn verify_record_launch_with(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    sender_identity: FileIdentity,
    command_line: &CommandLineEvidence,
    binding_is_current: impl FnOnce(&super::model::PackageLauncherBinding) -> bool,
) -> LaunchVerification {
    let Some(spec) = record.launch_spec.as_ref() else {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper);
    };
    if spec.wrappers.len() > 16 || spec.environment.len() > 128 {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper);
    }
    if !spec.runtime_executable.same_file(sender_identity) {
        return LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch);
    }
    if spec
        .package_launcher
        .as_ref()
        .is_some_and(|binding| !binding_is_current(binding))
    {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::LauncherBindingChanged);
    }
    if !literal_file_identities_are_current(spec) {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::ProtectedPayloadMismatch);
    }

    match classify_launch_authority(record, index, spec) {
        LaunchAuthority::DedicatedExecutable => verify_dedicated(command_line, spec),
        LaunchAuthority::ProtectedPayload => verify_protected_payload(command_line, spec),
        LaunchAuthority::DynamicOnly => {
            LaunchVerification::InsufficientEvidence(LaunchFailure::DynamicOnlyContract)
        }
        LaunchAuthority::Ambiguous => {
            LaunchVerification::InsufficientEvidence(LaunchFailure::AmbiguousDesktopAssociation)
        }
    }
}
