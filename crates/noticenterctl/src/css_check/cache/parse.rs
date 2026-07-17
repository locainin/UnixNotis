use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt as _;

use super::super::files::format_display_path;
use super::super::policy::parsing_error_hint;
use super::super::report::{CssCheckCategory, CssCheckDiagnostic};
use super::model::{CachedDiagnosticSource, CachedParseDiagnostic, CssParseWorkItem};

const VALIDATOR_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const MAX_VALIDATOR_OUTPUT_BYTES: usize = 64 * 1024;

pub(super) fn source_line_text(path: Option<&Path>, line_number: usize) -> Option<String> {
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
    parse_css_file_with_validator(work_item, &validator)
}

pub(super) fn parse_css_file_with_validator(
    work_item: &CssParseWorkItem,
    validator: &Path,
) -> Result<Vec<CachedParseDiagnostic>> {
    // An explicit helper path keeps process handling testable without hidden global overrides
    let output = run_css_validator(validator, &work_item.load_path, VALIDATOR_TIMEOUT)?;
    if !output.status.success() {
        return Err(anyhow!(
            "CSS validator exited with {}: {}",
            output.status,
            unixnotis_core::util::log_snippet(&String::from_utf8_lossy(&output.stderr))
        ));
    }

    decode_validator_report(&output.stdout, work_item)
}

#[derive(Debug)]
pub(super) struct ValidatorOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

pub(super) fn run_css_validator(
    validator: &Path,
    load_path: &Path,
    timeout: Duration,
) -> Result<ValidatorOutput> {
    let mut child = Command::new(validator)
        .arg("--json-path")
        .arg(load_path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("start CSS validator {}", validator.display()))?;
    // A malformed stylesheet must not leave diagnostics waiting forever
    let Some(status) = child
        .wait_timeout(timeout)
        .context("wait for CSS validator")?
    else {
        child.kill().context("stop timed-out CSS validator")?;
        child.wait().context("reap timed-out CSS validator")?;
        return Err(anyhow!(
            "CSS validator exceeded its {} second deadline",
            timeout.as_secs_f64()
        ));
    };

    // Read only after exit because the managed helper keeps each pipe below its OS buffer budget
    let stdout = read_bounded_pipe(
        child.stdout.take(),
        MAX_VALIDATOR_OUTPUT_BYTES,
        "CSS validator stdout",
    )?;
    let stderr = read_bounded_pipe(
        child.stderr.take(),
        MAX_VALIDATOR_OUTPUT_BYTES,
        "CSS validator stderr",
    )?;
    Ok(ValidatorOutput {
        status,
        stdout,
        stderr,
    })
}

pub(super) fn read_bounded_pipe(
    pipe: Option<impl std::io::Read>,
    limit: usize,
    label: &str,
) -> Result<Vec<u8>> {
    let Some(pipe) = pipe else {
        return Err(anyhow!("{label} pipe was unavailable"));
    };
    let take_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    pipe.take(take_limit)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {label}"))?;
    if bytes.len() > limit {
        return Err(anyhow!("{label} exceeded {limit} bytes"));
    }
    Ok(bytes)
}

pub(super) fn decode_validator_report(
    bytes: &[u8],
    work_item: &CssParseWorkItem,
) -> Result<Vec<CachedParseDiagnostic>> {
    let report: ValidatorReport =
        serde_json::from_slice(bytes).context("decode CSS validator report")?;
    if !report.available {
        return Err(anyhow!(
            "GTK CSS validation is unavailable: {}",
            report.error.as_deref().map_or_else(
                || "validator did not provide a reason".to_string(),
                unixnotis_core::util::log_snippet
            )
        ));
    }

    let mut diagnostics = report
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
        .collect::<Vec<_>>();
    if report.truncated {
        // One stable finding makes a capped report visible without repeating untrusted text
        diagnostics.push(CachedParseDiagnostic {
            source: CachedDiagnosticSource::TopLevel,
            line: None,
            column: None,
            message: "additional GTK CSS diagnostics were omitted after the safety limit"
                .to_string(),
            hint: None,
        });
    }
    Ok(diagnostics)
}

#[derive(Deserialize)]
struct ValidatorReport {
    available: bool,
    error: Option<String>,
    truncated: bool,
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

pub(super) fn classify_cached_source_path(
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
