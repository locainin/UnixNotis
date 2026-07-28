//! Evidence-based launch verification with explicit uncertainty and contradiction

use std::collections::HashSet;
use std::path::Path;

use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::super::sender::{CommandLineEvidence, CommandLineQuality};
use super::model::{
    DesktopIdentityIndex, DesktopRecord, LaunchArgument, LaunchAuthority, LaunchFailure,
    LaunchSpec, LaunchVerification, LiteralArgument, VerifiedLaunch,
};
use super::names::normalize_desktop_id;

pub(super) fn verify_record_launch(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    sender_identity: FileIdentity,
    command_line: &CommandLineEvidence,
) -> LaunchVerification {
    let Some(spec) = record.launch_spec.as_ref() else {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper);
    };
    if spec.wrappers.len() > 16 || spec.environment.len() > 128 {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper);
    }
    if !spec.executable.same_file(sender_identity) {
        return LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch);
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

fn classify_launch_authority(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    spec: &LaunchSpec,
) -> LaunchAuthority {
    if spec.arguments.iter().any(is_protected_payload) {
        return LaunchAuthority::ProtectedPayload;
    }

    let distinct_ids = index
        .records_for_executable(spec.executable)
        .into_iter()
        .filter(|candidate| !record.system_origin || candidate.system_origin)
        .map(|candidate| normalize_desktop_id(&candidate.id))
        .collect::<HashSet<_>>();
    if distinct_ids.len() == 1 {
        return LaunchAuthority::DedicatedExecutable;
    }

    if spec.arguments.iter().all(is_dynamic_or_option) {
        LaunchAuthority::DynamicOnly
    } else {
        LaunchAuthority::Ambiguous
    }
}

fn verify_dedicated(command_line: &CommandLineEvidence, spec: &LaunchSpec) -> LaunchVerification {
    match command_line.quality {
        // The live executable remains authoritative when argv memory is absent or rewritten
        CommandLineQuality::RewrittenProcessTitle
        | CommandLineQuality::Truncated
        | CommandLineQuality::Unavailable => {
            LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable)
        }
        CommandLineQuality::Structured => {
            if required_fixed_arguments_present(spec, &command_line.argv) {
                LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable)
            } else {
                LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch)
            }
        }
    }
}

fn verify_protected_payload(
    command_line: &CommandLineEvidence,
    spec: &LaunchSpec,
) -> LaunchVerification {
    match command_line.quality {
        CommandLineQuality::Unavailable | CommandLineQuality::Truncated => {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::MissingCommandLine);
        }
        CommandLineQuality::RewrittenProcessTitle => {
            return LaunchVerification::InsufficientEvidence(
                LaunchFailure::UnstructuredCommandLine,
            );
        }
        CommandLineQuality::Structured => {}
    }

    let actual = command_line.argv.get(1..).unwrap_or_default();
    for argument in &spec.arguments {
        let LaunchArgument::Literal(literal) = argument else {
            continue;
        };
        if literal.file.is_some()
            && !actual
                .iter()
                .any(|value| literal_file_matches(literal, value))
        {
            return LaunchVerification::DefinitiveMismatch(LaunchFailure::ProtectedPayloadMismatch);
        }
    }
    if !required_fixed_arguments_present(spec, &command_line.argv) {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch);
    }

    LaunchVerification::Verified(VerifiedLaunch::ProtectedPayload)
}

fn required_fixed_arguments_present(spec: &LaunchSpec, argv: &[Vec<u8>]) -> bool {
    let actual = argv.get(1..).unwrap_or_default();
    spec.arguments.iter().all(|argument| {
        let LaunchArgument::Literal(literal) = argument else {
            return true;
        };
        if literal.file.is_some() {
            actual
                .iter()
                .any(|value| literal_file_matches(literal, value))
        } else {
            actual.iter().any(|value| value == &literal.value)
        }
    })
}

fn literal_file_matches(literal: &LiteralArgument, actual: &[u8]) -> bool {
    let Some((_expected_path, expected_identity)) = literal.file.as_ref() else {
        return false;
    };
    let Ok(actual) = std::str::from_utf8(actual) else {
        return false;
    };
    executable_evidence_for_path(Path::new(actual))
        .is_some_and(|evidence| evidence.identity.same_file(*expected_identity))
}

fn literal_file_identities_are_current(spec: &LaunchSpec) -> bool {
    spec.arguments.iter().all(|argument| {
        let LaunchArgument::Literal(LiteralArgument {
            file: Some((path, expected)),
            ..
        }) = argument
        else {
            return true;
        };
        executable_evidence_for_path(path).is_some_and(|evidence| {
            evidence.identity.same_file(*expected) && evidence.identity.is_system_managed()
        })
    })
}

fn is_protected_payload(argument: &LaunchArgument) -> bool {
    matches!(
        argument,
        LaunchArgument::Literal(LiteralArgument {
            file: Some(_),
            value,
        }) if !value.starts_with(b"-")
    )
}

fn is_dynamic_or_option(argument: &LaunchArgument) -> bool {
    match argument {
        LaunchArgument::FieldCode(_) | LaunchArgument::OptionalIcon { .. } => true,
        LaunchArgument::Literal(literal) => literal.value.starts_with(b"-"),
    }
}

#[cfg(test)]
#[path = "tests/verification.rs"]
mod tests;
