//! Service-manager refresh execution after artifact changes

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

use crate::paths::format_with_home;
use crate::service_manager::{CommandSpec, S6DatabaseRefresh, ServiceArtifactRefresh};

use super::super::super::{log_line, ActionContext};
use super::dirs::ensure_directory_without_symlink;
use super::lifecycle::run_command_spec;

pub(in crate::actions::install) fn refresh_service_artifacts(
    ctx: &mut ActionContext,
) -> Result<()> {
    let Some(refresh) = ctx.paths.service.refresh_after_artifact_change() else {
        // Managers like dinit and runit discover changed artifacts during normal start
        log_line(
            ctx,
            format!(
                "Skipping {} refresh because this backend refreshes on start",
                ctx.paths.service.manager_label()
            ),
        );
        return Ok(());
    };

    match refresh {
        ServiceArtifactRefresh::Command(spec) => {
            // Single-command managers still use the normal command runner and logging
            log_line(
                ctx,
                format!("Refreshing {}", ctx.paths.service.manager_label()),
            );
            run_command_spec(ctx, &spec)
        }
        ServiceArtifactRefresh::S6Database(plan) => refresh_s6_database(ctx, &plan),
    }
}

fn refresh_s6_database(ctx: &mut ActionContext, plan: &S6DatabaseRefresh) -> Result<()> {
    // Source and rc roots are created through the hardened directory helper
    ensure_directory_without_symlink(&plan.source_root()).with_context(|| {
        format!(
            "failed to prepare s6 source root {}",
            format_with_home(&plan.source_root())
        )
    })?;
    ensure_directory_without_symlink(&plan.rc_root()).with_context(|| {
        format!(
            "failed to prepare s6 database root {}",
            format_with_home(&plan.rc_root())
        )
    })?;

    let compiled = next_s6_compiled_database(plan)?;
    // Each compile target is unique so failed refreshes never reuse a partial database
    log_line(
        ctx,
        format!(
            "Compiling s6 user database at {}",
            format_with_home(&compiled)
        ),
    );
    let compile = plan.compile_command(&compiled);
    run_command_spec(ctx, &compile)?;

    let update_outcome = if path_is_live_directory(plan.live_root()) {
        log_line(
            ctx,
            format!(
                "Updating live s6 user database at {}",
                format_with_home(plan.live_root())
            ),
        );
        let update = plan.update_command(&compiled);
        run_s6_database_update(ctx, &update)?
    } else {
        // Readiness normally blocks this path, but direct callers still get a precise failure
        return Err(anyhow!(
            "s6 live directory {} is missing; start local s6 supervision before refreshing UnixNotis",
            format_with_home(plan.live_root())
        ));
    };

    // Match the Artix and upstream pattern: update live state, then switch the stable link
    // Exit codes 1 and 2 still mean the live database moved, so the stable link must move too
    switch_s6_compiled_link(plan, &compiled)?;
    update_outcome.into_result()?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum S6UpdateOutcome {
    Clean,
    SwitchedWithTransitionFailure {
        reason: &'static str,
        diagnostic: Option<String>,
    },
}

impl S6UpdateOutcome {
    fn into_result(self) -> Result<()> {
        match self {
            Self::Clean => Ok(()),
            Self::SwitchedWithTransitionFailure { reason, diagnostic } => {
                let diagnostic = s6_diagnostic_suffix(diagnostic.as_deref());
                Err(anyhow!(
                    "s6-rc-update switched the live database, but {reason}{diagnostic}"
                ))
            }
        }
    }
}

fn run_s6_database_update(ctx: &mut ActionContext, spec: &CommandSpec) -> Result<S6UpdateOutcome> {
    let mut command = spec.to_command();
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("command failed to start: {}", spec.label()))?;
    let diagnostic = s6_stderr_diagnostic(&output.stderr);

    match output.status.code() {
        Some(0) => Ok(S6UpdateOutcome::Clean),
        Some(1) => {
            // s6 docs: database switched, but some service transitions failed
            log_s6_switched_failure(
                ctx,
                "some service transitions failed",
                diagnostic.as_deref(),
            );
            Ok(S6UpdateOutcome::SwitchedWithTransitionFailure {
                reason: "some service transitions failed",
                diagnostic,
            })
        }
        Some(2) => {
            // s6 docs: database switched, but the transition timed out
            log_s6_switched_failure(ctx, "service transition timed out", diagnostic.as_deref());
            Ok(S6UpdateOutcome::SwitchedWithTransitionFailure {
                reason: "the service transition timed out",
                diagnostic,
            })
        }
        Some(9) => {
            log_s6_update_diagnostic(ctx, output.status, diagnostic.as_deref());
            let diagnostic = s6_diagnostic_suffix(diagnostic.as_deref());
            Err(anyhow!(
                "s6-rc-update failed service transitions and did not switch the live database{diagnostic}"
            ))
        }
        Some(10) => {
            log_s6_update_diagnostic(ctx, output.status, diagnostic.as_deref());
            let diagnostic = s6_diagnostic_suffix(diagnostic.as_deref());
            Err(anyhow!(
                "s6-rc-update timed out and did not switch the live database{diagnostic}"
            ))
        }
        Some(code) => {
            log_s6_update_diagnostic(ctx, output.status, diagnostic.as_deref());
            Err(anyhow!(
                "s6-rc-update failed with exit code {code}; live database switch state is unknown{}",
                s6_diagnostic_suffix(diagnostic.as_deref())
            ))
        }
        None => {
            log_s6_update_diagnostic(ctx, output.status, diagnostic.as_deref());
            Err(anyhow!(
                "s6-rc-update terminated without an exit code; live database switch state is unknown{}",
                s6_diagnostic_suffix(diagnostic.as_deref())
            ))
        }
    }
}

fn log_s6_switched_failure(ctx: &mut ActionContext, reason: &str, diagnostic: Option<&str>) {
    // Known switched-database outcomes get one precise warning instead of a generic failure line
    log_line(
        ctx,
        format!(
            "Warning: s6-rc-update switched database but {reason}{}",
            s6_diagnostic_suffix(diagnostic)
        ),
    );
}

fn log_s6_update_diagnostic(ctx: &mut ActionContext, status: ExitStatus, diagnostic: Option<&str>) {
    let Some(diagnostic) = diagnostic else {
        return;
    };
    let status_label = status.code().map_or_else(
        || "without exit code".to_string(),
        |code| format!("with exit code {code}"),
    );
    // Keep the detailed s6 message available without dumping the full command stderr stream
    log_line(
        ctx,
        format!("Warning: s6-rc-update failed {status_label}: {diagnostic}"),
    );
}

fn s6_diagnostic_suffix(diagnostic: Option<&str>) -> String {
    diagnostic.map_or_else(String::new, |line| format!(": {line}"))
}

pub(in crate::actions::install) fn s6_stderr_diagnostic(stderr: &[u8]) -> Option<String> {
    const MAX_DIAGNOSTIC_LEN: usize = 240;

    // Only the first useful line is shown so noisy s6 tools do not flood the progress log
    String::from_utf8_lossy(stderr)
        .lines()
        .map(sanitize_diagnostic_line)
        .find(|line| !line.is_empty())
        .map(|line| truncate_diagnostic(line, MAX_DIAGNOSTIC_LEN))
}

pub(in crate::actions::install) fn sanitize_diagnostic_line(line: &str) -> String {
    strip_ansi_csi_sequences(line)
        .chars()
        .filter_map(|ch| match ch {
            // Tabs are spacing, but raw tabs can still make compact TUI logs hard to scan
            '\t' => Some(' '),
            // Drop escape bytes and every other control character before the line hits the TUI
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub(in crate::actions::install) fn strip_ansi_csi_sequences(line: &str) -> String {
    let mut sanitized = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            // CSI sequences are ASCII-only terminal controls like ESC[31m or ESC[0m
            chars.next();
            for csi_ch in chars.by_ref() {
                // Final bytes end the CSI sequence; everything before that is parameter text
                if ('\u{40}'..='\u{7e}').contains(&csi_ch) {
                    break;
                }
            }
            continue;
        }
        sanitized.push(ch);
    }

    sanitized
}

pub(in crate::actions::install) fn truncate_diagnostic(mut line: String, max_len: usize) -> String {
    if line.len() <= max_len {
        return line;
    }

    const ELLIPSIS: &str = "...";
    if max_len <= ELLIPSIS.len() {
        // Very small budgets still need valid UTF-8 and must not exceed the caller limit
        return ELLIPSIS[..max_len].to_string();
    }

    // Reserve room for the ellipsis so max_len means final rendered bytes, not body bytes
    let max_body_len = max_len - ELLIPSIS.len();
    // truncate() requires a UTF-8 boundary, so walk back until the boundary is valid
    let mut end = max_body_len;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line.truncate(end);
    line.push_str(ELLIPSIS);
    line
}

fn next_s6_compiled_database(plan: &S6DatabaseRefresh) -> Result<PathBuf> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let candidate = plan.rc_root().join(format!(
        "compiled-unixnotis-{}-{suffix}",
        std::process::id()
    ));
    if fs::symlink_metadata(&candidate).is_ok() {
        return Err(anyhow!(
            "refusing to reuse existing s6 compiled database path {}",
            format_with_home(&candidate)
        ));
    }
    Ok(candidate)
}

fn switch_s6_compiled_link(plan: &S6DatabaseRefresh, compiled: &Path) -> Result<()> {
    let link = plan.compiled_link();
    reject_unsafe_existing_compiled_link(&link)?;

    let temp_link = plan
        .rc_root()
        .join(format!(".compiled-unixnotis-next-{}", std::process::id()));
    // Only UnixNotis-created symlink temp files can be reused between failed attempts
    remove_stale_temp_link(&temp_link)?;

    #[cfg(unix)]
    {
        // s6-rc-init expects the boot database path to be a symlink to a compiled database
        unix_fs::symlink(compiled, &temp_link)
            .with_context(|| format!("failed to create {}", format_with_home(&temp_link)))?;
        fs::rename(&temp_link, &link).with_context(|| {
            format!(
                "failed to atomically switch s6 compiled database symlink {}",
                format_with_home(&link)
            )
        })?;
    }

    #[cfg(not(unix))]
    {
        let _ = compiled;
        let _ = temp_link;
        return Err(anyhow!(
            "s6 database symlinks require Unix filesystem support"
        ));
    }

    Ok(())
}

fn reject_unsafe_existing_compiled_link(link: &Path) -> Result<()> {
    match fs::symlink_metadata(link) {
        // Existing compiled links are expected; regular files or directories are user state
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(anyhow!(
            "refusing to replace non-symlink s6 compiled database path {}",
            format_with_home(link)
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(link)))
        }
    }
}

fn remove_stale_temp_link(temp_link: &Path) -> Result<()> {
    match fs::symlink_metadata(temp_link) {
        // Removing only a symlink keeps a hostile or accidental directory from being replaced
        Ok(metadata) if metadata.file_type().is_symlink() => fs::remove_file(temp_link)
            .with_context(|| format!("failed to remove {}", format_with_home(temp_link))),
        Ok(_) => Err(anyhow!(
            "refusing to replace non-symlink temp s6 database path {}",
            format_with_home(temp_link)
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(temp_link)))
        }
    }
}

fn path_is_live_directory(path: &Path) -> bool {
    fs::metadata(path)
        // s6 live roots are normally symlinks, and the symlink name is the command contract
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
}
