use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::super::files::format_display_path;
use super::super::policy::parsing_error_hint;
use super::super::report::{CssCheckCategory, CssCheckDiagnostic};
use super::model::{CachedDiagnosticSource, CachedParseDiagnostic, CssParseWorkItem};

fn source_line_text(path: Option<&Path>, line_number: usize) -> Option<String> {
    let path = path?;
    if line_number == 0 {
        return None;
    }
    // Read only when a parser error needs a hint
    let contents = fs::read_to_string(path).ok()?;
    contents
        .lines()
        // GTK line numbers start at one
        .nth(line_number.saturating_sub(1))
        .map(str::to_string)
}

pub(in super::super) fn parse_css_file_with_gtk(
    work_item: &CssParseWorkItem,
) -> Result<Vec<CachedParseDiagnostic>> {
    // GTK lives in an installer-managed helper so ordinary control calls stay lightweight
    let validator = css_validator_binary()?;
    let output = Command::new(&validator)
        .arg("--json-path")
        .arg(&work_item.load_path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .with_context(|| format!("start CSS validator {}", validator.display()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "CSS validator exited with {}: {}",
            output.status,
            unixnotis_core::util::log_snippet(&String::from_utf8_lossy(&output.stderr))
        ));
    }

    let report: ValidatorReport =
        serde_json::from_slice(&output.stdout).context("decode CSS validator report")?;
    if !report.available {
        return Err(anyhow!(
            "GTK CSS validation is unavailable: {}",
            report.error.as_deref().map_or_else(
                || "validator did not provide a reason".to_string(),
                unixnotis_core::util::log_snippet
            )
        ));
    }

    Ok(report
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            // Line hints stay tied to the exact file GTK blamed
            let hint = diagnostic
                .source
                .as_deref()
                .and_then(|path| source_line_text(Some(path), diagnostic.line))
                .and_then(|line_text| parsing_error_hint(&line_text));
            CachedParseDiagnostic {
                source: classify_cached_source_path(
                    diagnostic.source.as_deref(),
                    &work_item.canonical_path,
                ),
                line: Some(diagnostic.line),
                column: Some(diagnostic.column),
                message: diagnostic.message,
                hint,
            }
        })
        .collect())
}

#[derive(Deserialize)]
struct ValidatorReport {
    available: bool,
    error: Option<String>,
    diagnostics: Vec<ValidatorDiagnostic>,
}

#[derive(Deserialize)]
struct ValidatorDiagnostic {
    source: Option<PathBuf>,
    line: usize,
    column: usize,
    message: String,
}

fn css_validator_binary() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().context("resolve noticenterctl executable")?;
    css_validator_binary_from(&current_exe)
}

pub(super) fn css_validator_binary_from(current_exe: &Path) -> Result<PathBuf> {
    let parent = current_exe
        .parent()
        .ok_or_else(|| anyhow!("noticenterctl executable has no parent directory"))?;
    // Installed binaries share one directory while Cargo test binaries live under deps
    let mut candidates = vec![parent.join("unixnotis-css-validate")];
    if parent.file_name().is_some_and(|name| name == "deps") {
        if let Some(target_dir) = parent.parent() {
            candidates.push(target_dir.join("unixnotis-css-validate"));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| is_executable_regular_file(candidate))
        .ok_or_else(|| {
            anyhow!("unixnotis-css-validate is missing beside noticenterctl; reinstall UnixNotis")
        })
}

pub(super) fn is_executable_regular_file(path: &Path) -> bool {
    // Symlinks are excluded so the helper identity cannot escape the managed bin directory
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub(in super::super) fn render_cached_diagnostics(
    diagnostics: &[CachedParseDiagnostic],
    work_item: &CssParseWorkItem,
    config_dir: &Path,
    display_root: &str,
) -> Vec<CssCheckDiagnostic> {
    // Top-level errors should always point at the current logical input path
    let top_level_display = format_display_path(config_dir, display_root, &work_item.load_path);
    diagnostics
        .iter()
        .map(|diagnostic| {
            let display_path = match &diagnostic.source {
                CachedDiagnosticSource::TopLevel => top_level_display.clone(),
                CachedDiagnosticSource::Path(path) => {
                    format_display_path(config_dir, display_root, path)
                }
                CachedDiagnosticSource::Data => "<data>".to_string(),
            };

            CssCheckDiagnostic::error(
                CssCheckCategory::Parse,
                display_path,
                diagnostic.line,
                diagnostic.column,
                diagnostic.message.clone(),
                diagnostic.hint.clone(),
            )
        })
        .collect()
}

fn classify_cached_source_path(
    source_path: Option<&Path>,
    current_file: &Path,
) -> CachedDiagnosticSource {
    // Missing source info still needs a stable bucket in the cached form
    let Some(source_path) = source_path else {
        return CachedDiagnosticSource::Data;
    };

    // Imported files should only be treated as top-level when they resolve back to the same file
    let normalized_source =
        fs::canonicalize(source_path).unwrap_or_else(|_| source_path.to_path_buf());
    if normalized_source == current_file {
        return CachedDiagnosticSource::TopLevel;
    }

    CachedDiagnosticSource::Path(source_path.to_path_buf())
}
