//! Desktop `Exec` template parsing and process-command matching

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gio::prelude::AppInfoExt;

use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::model::{FieldCode, LaunchArgument, LaunchSpec, LiteralArgument};

const MAX_EXEC_TEMPLATE_BYTES: usize = 16 * 1024;
const MAX_EXEC_TEMPLATE_ARGUMENTS: usize = 128;
const MAX_PROCESS_ARGUMENTS: usize = 256;

pub(super) fn build_launch_spec(
    desktop: &gio::DesktopAppInfo,
    desktop_path: &Path,
    executable: FileIdentity,
) -> Option<LaunchSpec> {
    let template = desktop.string("Exec")?;
    if template.len() > MAX_EXEC_TEMPLATE_BYTES {
        return None;
    }
    let words = shell_words::split(template.as_str()).ok()?;
    if words.is_empty() || words.len() > MAX_EXEC_TEMPLATE_ARGUMENTS {
        return None;
    }

    let mut arguments = Vec::with_capacity(words.len().saturating_sub(1));
    let mut literal_files_are_system_managed = true;
    for word in words.into_iter().skip(1) {
        let argument = match word.as_str() {
            "%f" => LaunchArgument::FieldCode(FieldCode::File),
            "%F" => LaunchArgument::FieldCode(FieldCode::Files),
            "%u" => LaunchArgument::FieldCode(FieldCode::Url),
            "%U" => LaunchArgument::FieldCode(FieldCode::Urls),
            "%c" => literal_argument(desktop.display_name().as_bytes().to_vec()),
            "%k" => literal_argument(desktop_path.as_os_str().as_encoded_bytes().to_vec()),
            "%i" => LaunchArgument::OptionalIcon {
                name: desktop
                    .string("Icon")
                    .map_or_else(String::new, |icon| icon.to_string()),
            },
            _ => {
                let literal = percent_literal(&word)?;
                let literal = literal_argument(literal.into_bytes());
                if let LaunchArgument::Literal(literal) = &literal {
                    if let Some((_path, identity)) = &literal.file {
                        if !identity.is_system_managed() {
                            literal_files_are_system_managed = false;
                        }
                    } else if literal_path_candidate(&literal.value) {
                        // An unresolved application path cannot support system association
                        literal_files_are_system_managed = false;
                    }
                }
                literal
            }
        };
        arguments.push(argument);
    }

    Some(LaunchSpec {
        executable,
        arguments,
        literal_files_are_system_managed,
    })
}

pub(super) fn launch_spec_matches_sender(
    spec: &LaunchSpec,
    sender_identity: FileIdentity,
    cmdline: &[Vec<u8>],
) -> bool {
    if !spec.executable.same_file(sender_identity)
        || cmdline.is_empty()
        || cmdline.len() > MAX_PROCESS_ARGUMENTS
    {
        return false;
    }
    if !literal_file_identities_are_current(spec) {
        return false;
    }
    let mut visited = HashSet::new();
    match_arguments(&spec.arguments, &cmdline[1..], 0, 0, &mut visited)
}

fn literal_argument(value: Vec<u8>) -> LaunchArgument {
    let file = std::str::from_utf8(&value)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .and_then(|path| {
            executable_evidence_for_path(&path).map(|evidence| (path, evidence.identity))
        });
    LaunchArgument::Literal(LiteralArgument { value, file })
}

fn literal_path_candidate(value: &[u8]) -> bool {
    // Slash-bearing non-option literals are application payload paths even when unresolved
    !value.starts_with(b"-") && value.contains(&b'/')
}

fn percent_literal(word: &str) -> Option<String> {
    let mut output = String::with_capacity(word.len());
    let mut characters = word.chars();
    while let Some(character) = characters.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        if characters.next()? != '%' {
            return None;
        }
        output.push('%');
    }
    Some(output)
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
            actual.get(actual_index) == Some(&literal.value)
                && match_arguments(
                    template,
                    actual,
                    template_index + 1,
                    actual_index + 1,
                    visited,
                )
        }
        LaunchArgument::OptionalIcon { name } => {
            match_arguments(template, actual, template_index + 1, actual_index, visited)
                || (actual
                    .get(actual_index)
                    .is_some_and(|value| value == b"--icon")
                    && actual
                        .get(actual_index + 1)
                        .is_some_and(|value| value == name.as_bytes())
                    && match_arguments(
                        template,
                        actual,
                        template_index + 1,
                        actual_index + 2,
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
        let values = actual
            .get(actual_index..actual_index + count)
            .unwrap_or_default();
        if !values.iter().all(|value| field_value_matches(code, value)) {
            break;
        }
        if match_arguments(
            template,
            actual,
            template_index + 1,
            actual_index + count,
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

#[cfg(test)]
#[path = "tests/launch.rs"]
mod tests;
