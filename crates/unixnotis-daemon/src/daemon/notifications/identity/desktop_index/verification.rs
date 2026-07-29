//! Evidence-based launch verification with explicit uncertainty and contradiction

use std::collections::HashSet;
use std::path::Path;

use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::super::sender::{CommandLineEvidence, CommandLineQuality};
use super::model::{
    DesktopIdentityIndex, DesktopRecord, FieldCode, LaunchArgument, LaunchAuthority, LaunchFailure,
    LaunchSpec, LaunchVerification, LiteralArgument, VerifiedLaunch,
};

const MAX_PROCESS_ARGUMENTS: usize = 256;

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

    if executable_contract_is_dedicated(record, index, spec) {
        return LaunchAuthority::DedicatedExecutable;
    }

    // Dynamic documents are safe only after the executable establishes the application
    if spec.arguments.iter().any(is_dynamic_document_field) {
        return LaunchAuthority::DynamicOnly;
    }

    LaunchAuthority::Ambiguous
}

fn executable_contract_is_dedicated(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    spec: &LaunchSpec,
) -> bool {
    record.system_origin
        && record.system_association
        && spec.executable.is_system_managed()
        && spec.executable.is_executable_regular()
        && record
            .desktop_provenance
            .same_application_source(&record.executable_provenance)
        && index.records_form_one_application_family(spec.executable, record.system_origin)
        // A file in argv[1] can be either a document or an interpreter's active program
        // Static desktop metadata cannot distinguish those roles without a protected payload
        && !spec.arguments.iter().any(is_dynamic_file_field)
        && !spec.arguments.iter().any(is_unprotected_fixed_payload)
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
            let actual = command_line.argv.get(1..).unwrap_or_default();
            if actual.len() <= MAX_PROCESS_ARGUMENTS
                && (spec.arguments.is_empty() || match_ordered_dedicated_contract(spec, actual))
            {
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

fn match_ordered_dedicated_contract(spec: &LaunchSpec, actual: &[Vec<u8>]) -> bool {
    let mut visited = HashSet::new();
    match_dedicated_arguments(&spec.arguments, actual, 0, 0, &mut visited)
}

fn match_dedicated_arguments(
    template: &[LaunchArgument],
    actual: &[Vec<u8>],
    template_index: usize,
    actual_index: usize,
    visited: &mut HashSet<(usize, usize)>,
) -> bool {
    if !visited.insert((template_index, actual_index)) {
        return false;
    }
    let Some(argument) = template.get(template_index) else {
        // Only standalone runtime switches are non-authoritative after the fixed contract
        return actual[actual_index..]
            .iter()
            .all(|value| value.starts_with(b"-"));
    };
    let next_template = template_index.saturating_add(1);
    let matches_expected = match argument {
        LaunchArgument::Literal(literal) => {
            actual
                .get(actual_index)
                .is_some_and(|value| value == &literal.value)
                && match_dedicated_arguments(
                    template,
                    actual,
                    next_template,
                    actual_index.saturating_add(1),
                    visited,
                )
        }
        LaunchArgument::OptionalIcon { name } => {
            match_dedicated_arguments(template, actual, next_template, actual_index, visited)
                || (actual
                    .get(actual_index)
                    .is_some_and(|value| value == b"--icon")
                    && actual
                        .get(actual_index.saturating_add(1))
                        .is_some_and(|value| value == name.as_bytes())
                    && match_dedicated_arguments(
                        template,
                        actual,
                        next_template,
                        actual_index.saturating_add(2),
                        visited,
                    ))
        }
        LaunchArgument::FieldCode(code) => match_dedicated_field(
            *code,
            template,
            actual,
            template_index,
            actual_index,
            visited,
        ),
    };
    if matches_expected {
        return true;
    }

    // Unknown positional values can select content, so only skip one self-contained option
    actual
        .get(actual_index)
        .is_some_and(|value| value.starts_with(b"-") && value != b"--icon")
        && match_dedicated_arguments(
            template,
            actual,
            template_index,
            actual_index.saturating_add(1),
            visited,
        )
}

fn match_dedicated_field(
    code: FieldCode,
    template: &[LaunchArgument],
    actual: &[Vec<u8>],
    template_index: usize,
    actual_index: usize,
    visited: &mut HashSet<(usize, usize)>,
) -> bool {
    let maximum = match code {
        FieldCode::File | FieldCode::Url => 1,
        FieldCode::Files | FieldCode::Urls => actual.len().saturating_sub(actual_index),
    };
    for count in 0..=maximum {
        let Some(end) = actual_index.checked_add(count) else {
            break;
        };
        let Some(values) = actual.get(actual_index..end) else {
            break;
        };
        if !values.iter().all(|value| field_value_matches(code, value)) {
            break;
        }
        if match_dedicated_arguments(
            template,
            actual,
            template_index.saturating_add(1),
            end,
            visited,
        ) {
            return true;
        }
    }
    false
}

fn match_ordered_exec_contract(spec: &LaunchSpec, actual: &[Vec<u8>]) -> bool {
    let mut visited = HashSet::new();
    match_arguments(&spec.arguments, actual, 0, 0, &mut visited)
}

fn match_arguments(
    template: &[LaunchArgument],
    actual: &[Vec<u8>],
    template_index: usize,
    actual_index: usize,
    visited: &mut HashSet<(usize, usize)>,
) -> bool {
    if !visited.insert((template_index, actual_index)) {
        return false;
    }
    let Some(argument) = template.get(template_index) else {
        return actual_index == actual.len();
    };
    match argument {
        LaunchArgument::Literal(literal) => {
            let matches = if literal.file.is_some() {
                actual
                    .get(actual_index)
                    .is_some_and(|value| literal_file_matches(literal, value))
            } else {
                actual.get(actual_index) == Some(&literal.value)
            };
            matches
                && match_arguments(
                    template,
                    actual,
                    template_index.saturating_add(1),
                    actual_index.saturating_add(1),
                    visited,
                )
        }
        LaunchArgument::OptionalIcon { name } => {
            match_arguments(
                template,
                actual,
                template_index.saturating_add(1),
                actual_index,
                visited,
            ) || (actual
                .get(actual_index)
                .is_some_and(|value| value == b"--icon")
                && actual
                    .get(actual_index.saturating_add(1))
                    .is_some_and(|value| value == name.as_bytes())
                && match_arguments(
                    template,
                    actual,
                    template_index.saturating_add(1),
                    actual_index.saturating_add(2),
                    visited,
                ))
        }
        LaunchArgument::FieldCode(code) => match_field_code(
            *code,
            template,
            actual,
            template_index,
            actual_index,
            visited,
        ),
    }
}

fn match_field_code(
    code: FieldCode,
    template: &[LaunchArgument],
    actual: &[Vec<u8>],
    template_index: usize,
    actual_index: usize,
    visited: &mut HashSet<(usize, usize)>,
) -> bool {
    let maximum = match code {
        FieldCode::File | FieldCode::Url => 1,
        FieldCode::Files | FieldCode::Urls => actual.len().saturating_sub(actual_index),
    };
    for count in 0..=maximum {
        let Some(end) = actual_index.checked_add(count) else {
            break;
        };
        let Some(values) = actual.get(actual_index..end) else {
            break;
        };
        if !values.iter().all(|value| field_value_matches(code, value)) {
            break;
        }
        if match_arguments(
            template,
            actual,
            template_index.saturating_add(1),
            end,
            visited,
        ) {
            return true;
        }
    }
    false
}

fn field_value_matches(code: FieldCode, value: &[u8]) -> bool {
    if value.is_empty() || value.starts_with(b"-") {
        return false;
    }
    match code {
        FieldCode::File | FieldCode::Files => true,
        FieldCode::Url | FieldCode::Urls => std::str::from_utf8(value)
            .ok()
            .is_some_and(|value| url::Url::parse(value).is_ok()),
    }
}

fn protected_payload_position_mismatch(spec: &LaunchSpec, actual: &[Vec<u8>]) -> bool {
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

const fn is_dynamic_document_field(argument: &LaunchArgument) -> bool {
    matches!(argument, LaunchArgument::FieldCode(_))
}

const fn is_dynamic_file_field(argument: &LaunchArgument) -> bool {
    matches!(
        argument,
        LaunchArgument::FieldCode(FieldCode::File | FieldCode::Files)
    )
}

fn is_unprotected_fixed_payload(argument: &LaunchArgument) -> bool {
    matches!(
        argument,
        LaunchArgument::Literal(literal)
            if !literal.value.starts_with(b"-") && literal.file.is_none()
    )
}

#[cfg(test)]
#[path = "tests/verification.rs"]
mod tests;
