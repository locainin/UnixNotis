//! Durable release staging, atomic activation, and rollback

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use unixnotis_core::filesystem::{
    copy_file_atomic, create_directory_all, read_regular_file_bounded, read_symlink,
    remove_directory_tree, remove_regular_file, remove_symlink_if_target,
    rename_directory_no_replace, replace_symlink_atomic, write_file_atomic, RemoveSymlinkOutcome,
    RenameDirectoryOutcome,
};

use crate::managed_binaries::is_managed_binary_name;
use crate::paths::InstallPaths;

use super::entrypoints::{
    apply_entrypoint_changes, plan_entrypoint_changes, rollback_entrypoint_changes,
};
use super::manifest::{
    build_manifest, manifest_bytes, read_manifest, verify_release_directory,
    InstalledReleaseManifest, INSTALLED_MANIFEST_FILE,
};

pub(in crate::actions::releases) const MAX_PENDING_MANIFEST_BYTES: u64 = 256 * 1024;
pub(in crate::actions::releases) const PENDING_RELEASE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Deserialize, Serialize)]
pub(in crate::actions::releases) struct PendingRelease {
    pub(in crate::actions::releases) schema_version: u32,
    pub(in crate::actions::releases) generation: String,
    pub(in crate::actions::releases) new_current: PathBuf,
    pub(in crate::actions::releases) previous_current: Option<PathBuf>,
    pub(in crate::actions::releases) legacy_entrypoints: Vec<String>,
    pub(in crate::actions::releases) created_entrypoints: Vec<String>,
}

pub fn install_release_generation_transaction<F, R, G, C>(
    paths: &InstallPaths,
    release_source: &Path,
    binaries: &[String],
    precommit: F,
    reserve_activation: R,
    reserved_check: C,
) -> Result<String>
where
    F: FnMut() -> Result<()>,
    R: FnMut() -> Result<G>,
    C: FnMut() -> Result<()>,
{
    install_release_generation_with_checks(
        paths,
        release_source,
        binaries,
        precommit,
        reserve_activation,
        reserved_check,
    )
}

fn install_release_generation_with_checks<F, R, G, C>(
    paths: &InstallPaths,
    release_source: &Path,
    binaries: &[String],
    mut precommit: F,
    mut reserve_activation: R,
    mut reserved_check: C,
) -> Result<String>
where
    F: FnMut() -> Result<()>,
    R: FnMut() -> Result<G>,
    C: FnMut() -> Result<()>,
{
    let sources = binaries
        .iter()
        .map(|name| (name.clone(), release_source.join(name)))
        .collect::<Vec<_>>();
    let manifest = build_manifest(&sources)?;
    let generation = format!("{}-{}", manifest.package_version, manifest.build_id);
    let release_dir = stage_release(paths, &generation, &sources, &manifest)?;
    verify_release_directory(&release_dir, &manifest)?;

    // The first check runs before reserving the broker name, so it proves that no
    // existing notification daemon owns the runtime boundary
    precommit().context("verify daemon state before binary layout mutation")?;

    // Hold activation exclusion before recovery, journaling, or entrypoint mutation
    // Later checks omit the broker owner because this reservation is expected
    let _activation_reservation =
        reserve_activation().context("reserve daemon activation before binary layout mutation")?;
    reserved_check().context("verify selected service before binary layout mutation")?;

    commit_staged_release(paths, binaries, &generation, &mut reserved_check)
}

fn commit_staged_release<C>(
    paths: &InstallPaths,
    binaries: &[String],
    generation: &str,
    reserved_check: &mut C,
) -> Result<String>
where
    C: FnMut() -> Result<()>,
{
    // Recovery can change current and entrypoints, so prove the selected service is still stopped
    if let Err(error) = reserved_check() {
        return Err(error.context("verify selected service before pending-release recovery"));
    }
    rollback_pending_release(paths).context("recover incomplete prior binary installation")?;
    let pending = plan_entrypoint_changes(paths, binaries, generation)?;
    let pending_path = paths.installed_pending_manifest()?;
    // Atomic publication synchronizes both the journal file and its parent directory
    // No live entrypoint may change before this durable recovery authority exists
    write_pending(&pending_path, &pending)?;

    // Planning and journal I/O may take time, so prove quiescence again at first mutation
    if let Err(error) = reserved_check() {
        return Err(rollback_with_context(paths, error));
    }
    if let Err(error) = apply_entrypoint_changes(paths, &pending) {
        return Err(rollback_with_context(
            paths,
            error.context("publish managed binary entrypoints"),
        ));
    }

    // Runtime state is sampled again immediately before the atomic generation switch
    if let Err(error) = reserved_check() {
        return Err(rollback_with_context(paths, error));
    }
    if let Err(error) =
        replace_symlink_atomic(&paths.installed_current_link()?, &pending.new_current)
    {
        return Err(rollback_with_context(
            paths,
            anyhow!(error).context("atomically switch installed release generation"),
        ));
    }

    // The journal covers every entrypoint mutation and the exact selected generation
    Ok(generation.to_string())
}

fn stage_release(
    paths: &InstallPaths,
    generation: &str,
    sources: &[(String, PathBuf)],
    manifest: &InstalledReleaseManifest,
) -> Result<PathBuf> {
    stage_release_with_copy(
        paths,
        generation,
        sources,
        manifest,
        |source, destination| copy_file_atomic(source, destination).map_err(anyhow::Error::from),
    )
}

pub(in crate::actions::releases) fn stage_release_with_copy<F>(
    paths: &InstallPaths,
    generation: &str,
    sources: &[(String, PathBuf)],
    manifest: &InstalledReleaseManifest,
    mut copy_binary: F,
) -> Result<PathBuf>
where
    F: FnMut(&Path, &Path) -> Result<()>,
{
    let releases = paths.installed_releases_dir()?;
    create_directory_all(&releases, 0o755).context("create installed releases directory")?;
    let final_dir = releases.join(generation);
    if final_dir.exists() {
        verify_release_directory(&final_dir, manifest)?;
        return Ok(final_dir);
    }

    let staging = releases.join(format!(".staging-{generation}-{}", std::process::id()));
    if staging.exists() {
        remove_directory_tree(&staging).context("remove abandoned release staging directory")?;
    }
    create_directory_all(&staging.join("bin"), 0o755)
        .context("create release staging directory")?;
    let stage_result = (|| {
        for (name, source) in sources {
            copy_binary(source, &staging.join("bin").join(name))
                .with_context(|| format!("stage release binary {name}"))?;
        }
        write_file_atomic(
            &staging.join(INSTALLED_MANIFEST_FILE),
            &manifest_bytes(manifest)?,
            0o644,
        )
        .context("write staged release manifest")?;
        verify_release_directory(&staging, manifest)?;
        match rename_directory_no_replace(&staging, &final_dir)? {
            RenameDirectoryOutcome::Renamed => Ok(()),
            RenameDirectoryOutcome::DestinationExists => {
                remove_directory_tree(&staging)?;
                verify_release_directory(&final_dir, manifest)
            }
            RenameDirectoryOutcome::SourceMissing => Err(anyhow!(
                "release staging directory disappeared before publication"
            )),
        }
    })();
    if stage_result.is_err() {
        let _cleanup = remove_directory_tree(&staging);
    }
    stage_result?;
    Ok(final_dir)
}

pub fn rollback_pending_release(paths: &InstallPaths) -> Result<bool> {
    let pending_path = paths.installed_pending_manifest()?;
    let Some(pending) = read_pending(&pending_path)? else {
        return Ok(false);
    };
    validate_pending_journal(&pending)?;
    rollback_release_state(paths, &pending)?;
    remove_regular_file(&pending_path).context("remove pending release manifest")?;
    Ok(true)
}

fn rollback_release_state(paths: &InstallPaths, pending: &PendingRelease) -> Result<()> {
    let current = paths.installed_current_link()?;
    let visible_current = read_symlink(&current)?;
    if visible_current.as_ref() == Some(&pending.new_current) {
        if let Some(previous) = pending.previous_current.as_ref() {
            // Rollback authority depends on the previous generation remaining byte-for-byte valid
            verify_release_target(paths, previous)
                .context("verify previous release generation before rollback")?;
            replace_symlink_atomic(&current, previous)
                .context("restore prior release generation")?;
        } else {
            match remove_symlink_if_target(&current, &pending.new_current)? {
                RemoveSymlinkOutcome::Removed | RemoveSymlinkOutcome::Missing => {}
                RemoveSymlinkOutcome::TargetMismatch(actual) => {
                    return Err(anyhow!(
                        "current release changed during rollback to {}",
                        actual.display()
                    ));
                }
            }
        }
    } else {
        let already_restored = pending.previous_current.as_ref().map_or_else(
            || visible_current.is_none(),
            |previous| visible_current.as_ref() == Some(previous),
        );
        if !already_restored {
            let actual = visible_current.map_or_else(
                || "missing".to_string(),
                |target| target.display().to_string(),
            );
            return Err(anyhow!(
                "current release changed during rollback to {actual}"
            ));
        }
        if let Some(previous) = pending.previous_current.as_ref() {
            // A crash may leave current restored while the journal still needs entrypoint cleanup
            verify_release_target(paths, previous)
                .context("verify already restored release generation during rollback")?;
        }
    }
    rollback_entrypoint_changes(paths, pending)?;
    Ok(())
}

pub fn commit_pending_release(paths: &InstallPaths) -> Result<bool> {
    let pending_path = paths.installed_pending_manifest()?;
    let Some(pending) = read_pending(&pending_path)? else {
        return Ok(false);
    };
    validate_pending_journal(&pending)?;
    if read_symlink(&paths.installed_current_link()?)?.as_ref() != Some(&pending.new_current) {
        return Err(anyhow!("installed release changed before readiness commit"));
    }
    verify_release_target(paths, &pending.new_current)
        .context("verify ready release generation before commit")?;
    // Retention is still reversible because current and previous generations remain untouched
    prune_release_generations(paths, &pending)
        .context("prune superseded installed release generations")?;
    remove_regular_file(&pending_path).context("remove committed pending release manifest")?;
    let rollback_generation = paths.installed_rollback_root()?.join(&pending.generation);
    if rollback_generation.exists() {
        // Scratch cleanup follows the journal commit point and cannot make activation fail
        let _cleanup = remove_directory_tree(&rollback_generation);
    }
    Ok(true)
}

pub(in crate::actions::releases) fn verify_release_target(
    paths: &InstallPaths,
    target: &Path,
) -> Result<InstalledReleaseManifest> {
    if !is_managed_current_target(target) {
        return Err(anyhow!(
            "current release link points outside the managed releases directory"
        ));
    }
    let release_dir = paths.installed_release_root()?.join(target);
    let manifest = read_manifest(&release_dir.join(INSTALLED_MANIFEST_FILE))?;
    let expected_generation = format!("{}-{}", manifest.package_version, manifest.build_id);
    if target.file_name().and_then(|name| name.to_str()) != Some(expected_generation.as_str()) {
        return Err(anyhow!(
            "current release directory does not match its manifest generation"
        ));
    }
    verify_release_directory(&release_dir, &manifest)?;
    Ok(manifest)
}

fn prune_release_generations(paths: &InstallPaths, pending: &PendingRelease) -> Result<()> {
    let releases = paths.installed_releases_dir()?;
    let retained = [
        pending.new_current.file_name(),
        pending
            .previous_current
            .as_ref()
            .and_then(|target| target.file_name()),
    ]
    .into_iter()
    .flatten()
    .collect::<std::collections::HashSet<_>>();
    for entry in fs::read_dir(&releases).context("read installed release generations")? {
        let entry = entry.context("read installed release generation entry")?;
        let file_name = entry.file_name();
        if retained.contains(file_name.as_os_str()) {
            continue;
        }
        let file_type = entry
            .file_type()
            .context("inspect installed release generation entry")?;
        if !file_type.is_dir() {
            return Err(anyhow!(
                "installed releases directory contains an unmanaged object: {}",
                entry.path().display()
            ));
        }
        if file_name.to_string_lossy().starts_with(".staging-") {
            remove_directory_tree(&entry.path()).context("remove abandoned release staging")?;
            continue;
        }
        let manifest = read_manifest(&entry.path().join(INSTALLED_MANIFEST_FILE))?;
        let expected_name = format!("{}-{}", manifest.package_version, manifest.build_id);
        if file_name != std::ffi::OsStr::new(&expected_name) {
            return Err(anyhow!(
                "installed release directory name does not match its manifest"
            ));
        }
        verify_release_directory(&entry.path(), &manifest)?;
        remove_directory_tree(&entry.path()).context("remove superseded release generation")?;
    }
    Ok(())
}

pub fn pending_release_has_runtime_rollback(paths: &InstallPaths) -> Result<bool> {
    let Some(pending) = read_pending(&paths.installed_pending_manifest()?)? else {
        return Ok(false);
    };
    validate_pending_journal(&pending)?;
    Ok(pending.previous_current.is_some() || !pending.legacy_entrypoints.is_empty())
}

pub fn pending_release_exists(paths: &InstallPaths) -> Result<bool> {
    Ok(read_pending(&paths.installed_pending_manifest()?)?.is_some())
}

pub(in crate::actions::releases) fn write_pending(
    path: &Path,
    pending: &PendingRelease,
) -> Result<()> {
    validate_pending_journal(pending)?;
    let bytes = serde_json::to_vec_pretty(pending).context("serialize pending release")?;
    write_file_atomic(path, &bytes, 0o600).context("write pending release manifest")
}

pub(in crate::actions::releases) fn read_pending(path: &Path) -> Result<Option<PendingRelease>> {
    match read_regular_file_bounded(path, MAX_PENDING_MANIFEST_BYTES) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .context("parse pending release manifest"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("read pending release manifest"),
    }
}

pub(in crate::actions::releases) fn validate_pending_targets(
    pending: &PendingRelease,
) -> Result<()> {
    let expected_new_current = PathBuf::from("releases").join(&pending.generation);
    if pending.new_current != expected_new_current
        || !is_managed_current_target(&pending.new_current)
        || pending
            .previous_current
            .as_ref()
            .is_some_and(|target| !is_managed_current_target(target))
    {
        return Err(anyhow!(
            "pending release contains an unmanaged current-link target"
        ));
    }
    let mut names = std::collections::HashSet::new();
    for name in pending
        .legacy_entrypoints
        .iter()
        .chain(&pending.created_entrypoints)
    {
        if !is_managed_binary_name(name) || !names.insert(name) {
            return Err(anyhow!(
                "pending release contains an invalid or duplicate binary entrypoint"
            ));
        }
    }
    Ok(())
}

fn validate_pending_journal(pending: &PendingRelease) -> Result<()> {
    if pending.schema_version != PENDING_RELEASE_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported pending release schema {}",
            pending.schema_version
        ));
    }
    validate_pending_targets(pending)
}

pub(in crate::actions::releases) fn is_managed_current_target(target: &Path) -> bool {
    let mut components = target.components();
    matches!(
        (components.next(), components.next(), components.next()),
        (
            Some(std::path::Component::Normal(root)),
            Some(std::path::Component::Normal(_generation)),
            None
        ) if root == "releases"
    )
}

fn rollback_with_context(paths: &InstallPaths, error: anyhow::Error) -> anyhow::Error {
    match rollback_pending_release(paths) {
        Ok(_rolled_back) => error,
        Err(rollback_error) => error.context(format!(
            "binary release rollback also failed: {rollback_error:#}"
        )),
    }
}
