//! Config-relative script dependency discovery for preset export
//!
//! This intentionally recognizes only direct shell source statements
//! Broad shell evaluation would execute user content and could recapture unrelated files

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use unixnotis_core::filesystem::ContainedPath;

use anyhow::{anyhow, Context, Result};

use super::super::config_root::SecureFileCapture;
use super::super::filesystem::read_relative_file_secure_bounded;

// Source scanning is only dependency metadata work, so a small cap prevents an oversized command
// file from consuming memory before the normal preset file limits are applied
pub(super) const MAX_SCANNED_SCRIPT_BYTES: u64 = 1024 * 1024;

pub(super) struct ScriptDependencyClosure {
    pub(super) paths: Vec<PathBuf>,
    pub(super) captures: BTreeMap<PathBuf, SecureFileCapture>,
}

#[derive(Clone, Copy)]
enum ScriptScanKind {
    // Entry files can be native executables that have no shell dependency syntax
    Entry,
    // A file reached through source syntax is shell input even when it has no shebang
    Sourced,
}

pub(super) enum SourceOperand {
    // Dependency path is proven to stay under the config root
    Portable(PathBuf),
    // Shell expansion decides the path only when the command runs
    RuntimeDynamic,
    // Absolute system files are supplied by the destination host
    AbsoluteSystem,
    // A script-dir path is known to leave the portable config tree
    UnsafeEscape,
    // Relative shell lookup depends on process state UnixNotis does not control
    AmbiguousRelative,
}

pub(super) fn collect_script_dependency_closure_from_root(
    root_fd: &OwnedFd,
    entry_scripts: &[PathBuf],
) -> Result<ScriptDependencyClosure> {
    let mut discovered = BTreeSet::new();
    let mut pending = VecDeque::new();
    let mut captures = BTreeMap::new();

    for entry in entry_scripts {
        // Config command resolution has already constrained entries to the config root
        // Normalizing again keeps this helper safe when called independently by tests
        let Some(entry) = normalize_relative_path(entry) else {
            continue;
        };
        if discovered.insert(entry.clone()) {
            let (contents, mode) =
                read_relative_file_secure_bounded(root_fd, &entry, MAX_SCANNED_SCRIPT_BYTES)
                    .with_context(|| {
                        format!(
                            "preset export cannot verify dependencies for script {}",
                            entry.display()
                        )
                    })?;
            captures.insert(entry.clone(), SecureFileCapture { contents, mode });
            pending.push_back((entry, ScriptScanKind::Entry));
        }
    }

    while let Some((script_relative, scan_kind)) = pending.pop_front() {
        let bytes = &captures
            .get(&script_relative)
            .expect("queued script capture must exist")
            .contents;
        let Ok(contents) = std::str::from_utf8(bytes) else {
            if matches!(scan_kind, ScriptScanKind::Sourced) || has_shell_shebang(bytes) {
                return Err(anyhow!(
                    "preset export cannot verify non-UTF-8 shell dependency {}",
                    script_relative.display()
                ));
            }
            // Native executables are valid preset files and do not contain shell source syntax
            continue;
        };

        if matches!(scan_kind, ScriptScanKind::Entry) && !has_shell_shebang(bytes) {
            // UTF-8 executables may be Python, Ruby, JavaScript, or plain native payloads
            // Only a shell shebang makes `source` syntax meaningful for an entry file
            continue;
        }

        // Own the small operand list before captures grow so no map entry stays borrowed
        let operands = source_operands(contents).collect::<Vec<_>>();
        for operand in operands {
            let dependency = match resolve_source_operand(&script_relative, &operand) {
                SourceOperand::Portable(dependency) => dependency,
                // Dynamic and absolute system dependencies intentionally remain destination concerns
                SourceOperand::RuntimeDynamic | SourceOperand::AbsoluteSystem => continue,
                SourceOperand::UnsafeEscape => {
                    return Err(anyhow!(
                        "script {} source operand {operand:?} escapes the UnixNotis config root",
                        script_relative.display()
                    ));
                }
                SourceOperand::AmbiguousRelative => {
                    return Err(anyhow!(
                        "script {} source operand {operand:?} depends on the runtime working directory; use $script_dir or ${{script_dir}}",
                        script_relative.display()
                    ));
                }
            };
            if discovered.insert(dependency.clone()) {
                let (contents, mode) = read_relative_file_secure_bounded(
                    root_fd,
                    &dependency,
                    MAX_SCANNED_SCRIPT_BYTES,
                )
                .with_context(|| {
                    format!(
                        "script {} sources missing, unsafe, or oversized preset dependency {}",
                        script_relative.display(),
                        dependency.display()
                    )
                })?;
                captures.insert(dependency.clone(), SecureFileCapture { contents, mode });
                // Newly found helpers are scanned too because shell libraries can source another local file
                pending.push_back((dependency, ScriptScanKind::Sourced));
            }
        }
    }

    Ok(ScriptDependencyClosure {
        paths: discovered.into_iter().collect(),
        captures,
    })
}

fn source_operands(contents: &str) -> impl Iterator<Item = String> + '_ {
    contents.lines().filter_map(|line| {
        let trimmed = line.trim_start();
        // Token matching naturally rejects blank lines and comments without a second parsing rule
        let words = shell_words::split(trimmed).ok()?;
        match words.as_slice() {
            [command, operand, ..] if command == "." || command == "source" => {
                Some(operand.clone())
            }
            _ => None,
        }
    })
}

pub(super) fn resolve_source_operand(script_relative: &Path, operand: &str) -> SourceOperand {
    let script_parent = script_relative.parent().unwrap_or_else(|| Path::new(""));
    let relative = if let Some(value) = operand.strip_prefix("$script_dir/") {
        script_parent.join(value)
    } else if let Some(value) = operand.strip_prefix("${script_dir}/") {
        script_parent.join(value)
    } else {
        // Expansions and absolute system libraries are runtime concerns rather than bundle paths
        if operand.contains('$') || operand.contains('`') {
            return SourceOperand::RuntimeDynamic;
        }
        if Path::new(operand).is_absolute() {
            return SourceOperand::AbsoluteSystem;
        }
        // Shells resolve ordinary relative operands from process state or PATH, not from config root
        return SourceOperand::AmbiguousRelative;
    };
    normalize_relative_path(&relative).map_or(SourceOperand::UnsafeEscape, SourceOperand::Portable)
}

fn has_shell_shebang(bytes: &[u8]) -> bool {
    let Some(first_line) = bytes.split(|byte| *byte == b'\n').next() else {
        return false;
    };
    let Ok(first_line) = std::str::from_utf8(first_line) else {
        return false;
    };
    let Some(command) = first_line.strip_prefix("#!") else {
        return false;
    };

    let mut words = command.split_ascii_whitespace();
    let Some(interpreter) = words.next().and_then(executable_name) else {
        return false;
    };
    if is_shell_name(interpreter) {
        return true;
    }
    if interpreter == "busybox" {
        return words
            .next()
            .and_then(executable_name)
            .is_some_and(is_shell_name);
    }
    if interpreter != "env" {
        return false;
    }

    env_interpreter(&mut words)
        .and_then(executable_name)
        .is_some_and(is_shell_name)
}

fn env_interpreter<'a>(words: &mut impl Iterator<Item = &'a str>) -> Option<&'a str> {
    // `env` may carry flags, flag values, and assignments before the interpreter
    while let Some(word) = words.next() {
        match word {
            "-u" | "--unset" | "-C" | "--chdir" => {
                // These options consume one following value before command lookup resumes
                words.next()?;
            }
            _ if word.starts_with('-') || word.contains('=') => {}
            _ => return Some(word),
        }
    }
    None
}

fn executable_name(word: &str) -> Option<&str> {
    Path::new(word).file_name()?.to_str()
}

fn is_shell_name(name: &str) -> bool {
    matches!(
        name,
        "sh" | "ash" | "bash" | "csh" | "dash" | "fish" | "ksh" | "mksh" | "tcsh" | "yash" | "zsh"
    )
}

pub(super) fn normalize_relative_path(path: &Path) -> Option<PathBuf> {
    let normalized = ContainedPath::resolve_relative("", path)
        .ok()?
        .relative()
        .to_path_buf();
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}
