//! Desktop `Exec` template parsing and process-command matching

use std::path::{Path, PathBuf};

use gio::prelude::AppInfoExt;

use super::super::executable::executable_evidence_for_path;
use super::model::{FieldCode, LaunchArgument, LaunchSpec, LiteralArgument};
use super::program::resolve_program;
use super::wrappers::normalize_launch_command;

const MAX_EXEC_TEMPLATE_BYTES: usize = 16 * 1024;
const MAX_EXEC_TEMPLATE_ARGUMENTS: usize = 128;

pub(super) fn build_launch_spec(
    desktop: &gio::DesktopAppInfo,
    desktop_path: &Path,
) -> Option<(PathBuf, LaunchSpec)> {
    let template = desktop.string("Exec")?;
    if template.len() > MAX_EXEC_TEMPLATE_BYTES {
        return None;
    }
    let words = shell_words::split(template.as_str()).ok()?;
    if words.is_empty() || words.len() > MAX_EXEC_TEMPLATE_ARGUMENTS {
        return None;
    }
    let normalized = normalize_launch_command(words).ok()?;
    let executable_path = resolve_program(Path::new(&normalized.executable))?;
    let executable = executable_evidence_for_path(&executable_path)?.identity;

    let mut arguments = Vec::with_capacity(normalized.arguments.len());
    let mut literal_files_are_system_managed = true;
    for word in normalized.arguments {
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

    Some((
        executable_path,
        LaunchSpec {
            executable,
            arguments,
            environment: normalized.environment,
            wrappers: normalized.wrappers,
            literal_files_are_system_managed,
        },
    ))
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

#[cfg(test)]
#[path = "tests/launch.rs"]
mod tests;
