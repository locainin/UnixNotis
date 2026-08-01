//! Protected payload and file-identity verification

use std::collections::HashSet;
use std::path::Path;

use super::super::super::executable::executable_evidence_for_path;
use super::super::super::sender::{CommandLineEvidence, CommandLineQuality};
use super::super::model::{
    LaunchArgument, LaunchFailure, LaunchSpec, LaunchVerification, LiteralArgument, VerifiedLaunch,
};
use super::authority::is_protected_payload;
use super::contract::{match_arguments, match_ordered_exec_contract};
use super::MAX_PROCESS_ARGUMENTS;

pub(super) fn verify_protected_payload(
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
    if actual.len() > MAX_PROCESS_ARGUMENTS {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnstructuredCommandLine);
    }
    if match_ordered_exec_contract(spec, actual) {
        return LaunchVerification::Verified(VerifiedLaunch::ProtectedPayload);
    }

    // A protected file in another argv slot is a decoy, not supporting evidence
    // Missing or replaced protected files are equally definitive for structured argv
    if protected_payload_position_mismatch(spec, actual) {
        return LaunchVerification::DefinitiveMismatch(LaunchFailure::ProtectedPayloadMismatch);
    }

    LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch)
}
pub(super) fn protected_payload_position_mismatch(spec: &LaunchSpec, actual: &[Vec<u8>]) -> bool {
    spec.arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| {
            let LaunchArgument::Literal(literal) = argument else {
                return None;
            };
            is_protected_payload(argument).then_some((index, literal))
        })
        .any(|(template_index, literal)| {
            !(0..actual.len()).any(|actual_index| {
                let mut visited = HashSet::new();
                match_arguments(
                    &spec.arguments[..template_index],
                    &actual[..actual_index],
                    0,
                    0,
                    &mut visited,
                ) && literal_file_matches(literal, &actual[actual_index])
            })
        })
}

pub(super) fn literal_file_matches(literal: &LiteralArgument, actual: &[u8]) -> bool {
    let Some((_expected_path, expected_identity)) = literal.file.as_ref() else {
        return false;
    };
    let Ok(actual) = std::str::from_utf8(actual) else {
        return false;
    };
    executable_evidence_for_path(Path::new(actual))
        .is_some_and(|evidence| evidence.identity.same_file(*expected_identity))
}

pub(super) fn literal_file_identities_are_current(spec: &LaunchSpec) -> bool {
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
