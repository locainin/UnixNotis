//! Ordered desktop launch-contract matching

use std::collections::HashSet;

use super::super::super::sender::{CommandLineEvidence, CommandLineQuality};
use super::super::model::{
    FieldCode, LaunchArgument, LaunchFailure, LaunchSpec, LaunchVerification, VerifiedLaunch,
};
use super::payload::literal_file_matches;
use super::MAX_PROCESS_ARGUMENTS;

pub(super) fn verify_dedicated(
    command_line: &CommandLineEvidence,
    spec: &LaunchSpec,
) -> LaunchVerification {
    let verified_launch = if spec.package_launcher.is_some() {
        VerifiedLaunch::PackageLauncherTarget
    } else {
        VerifiedLaunch::DedicatedExecutable
    };
    match command_line.quality {
        // Launcher targets require the original desktop contract to match observed runtime argv
        CommandLineQuality::RewrittenProcessTitle
        | CommandLineQuality::Truncated
        | CommandLineQuality::Unavailable
            if spec.package_launcher.is_some() =>
        {
            LaunchVerification::InsufficientEvidence(LaunchFailure::MissingCommandLine)
        }
        // An empty contract cannot distinguish an ordinary switch from an active payload
        CommandLineQuality::RewrittenProcessTitle
        | CommandLineQuality::Truncated
        | CommandLineQuality::Unavailable
            if spec.arguments.is_empty() =>
        {
            LaunchVerification::InsufficientEvidence(LaunchFailure::EmptyContractNeedsCommandLine)
        }
        // A nonempty package-backed contract still contributes identity when argv was rewritten
        CommandLineQuality::RewrittenProcessTitle
        | CommandLineQuality::Truncated
        | CommandLineQuality::Unavailable => LaunchVerification::Verified(verified_launch),
        CommandLineQuality::Structured => {
            let actual = command_line.argv.get(1..).unwrap_or_default();
            if actual.len() <= MAX_PROCESS_ARGUMENTS
                && match_ordered_dedicated_contract(spec, actual)
            {
                LaunchVerification::Verified(verified_launch)
            } else {
                LaunchVerification::InsufficientEvidence(LaunchFailure::RequiredArgumentMismatch)
            }
        }
    }
}
pub(super) fn match_ordered_dedicated_contract(spec: &LaunchSpec, actual: &[Vec<u8>]) -> bool {
    let mut visited = HashSet::new();
    match_dedicated_arguments(&spec.arguments, actual, 0, 0, &mut visited)
}

pub(super) fn match_dedicated_arguments(
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

pub(super) fn match_dedicated_field(
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

pub(super) fn match_ordered_exec_contract(spec: &LaunchSpec, actual: &[Vec<u8>]) -> bool {
    let mut visited = HashSet::new();
    match_arguments(&spec.arguments, actual, 0, 0, &mut visited)
}

pub(super) fn match_arguments(
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

pub(super) fn match_field_code(
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

pub(super) fn field_value_matches(code: FieldCode, value: &[u8]) -> bool {
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
