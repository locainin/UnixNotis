//! Service-manager refresh execution after artifact changes

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
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
    SwitchedWithTransitionFailure(&'static str),
}

impl S6UpdateOutcome {
    fn into_result(self) -> Result<()> {
        match self {
            Self::Clean => Ok(()),
            Self::SwitchedWithTransitionFailure(reason) => Err(anyhow!(
                "s6-rc-update switched the live database, but {reason}"
            )),
        }
    }
}

fn run_s6_database_update(ctx: &mut ActionContext, spec: &CommandSpec) -> Result<S6UpdateOutcome> {
    let mut command = spec.to_command();
    // Keep command output out of the TUI unless future callers add a structured log reader here
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("command failed to start: {}", spec.label()))?;

    match status.code() {
        Some(0) => Ok(S6UpdateOutcome::Clean),
        Some(1) => {
            // s6 docs: database switched, but some service transitions failed
            log_line(
                ctx,
                "Warning: s6-rc-update switched database but some service transitions failed",
            );
            Ok(S6UpdateOutcome::SwitchedWithTransitionFailure(
                "some service transitions failed",
            ))
        }
        Some(2) => {
            // s6 docs: database switched, but the transition timed out
            log_line(
                ctx,
                "Warning: s6-rc-update switched database but service transition timed out",
            );
            Ok(S6UpdateOutcome::SwitchedWithTransitionFailure(
                "the service transition timed out",
            ))
        }
        Some(9) => Err(anyhow!(
            "s6-rc-update failed service transitions and did not switch the live database"
        )),
        Some(10) => Err(anyhow!(
            "s6-rc-update timed out and did not switch the live database"
        )),
        Some(code) => Err(anyhow!(
            "s6-rc-update failed with exit code {code}; live database switch state is unknown"
        )),
        None => Err(anyhow!(
            "s6-rc-update terminated without an exit code; live database switch state is unknown"
        )),
    }
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
