//! Binary entrypoint planning, publication, and crash recovery

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    create_directory_all, create_symlink_if_missing, read_symlink, remove_directory_tree,
    remove_symlink_if_target, rename_regular_file_no_replace, CreateSymlinkOutcome,
    RemoveSymlinkOutcome, RenameRegularFileOutcome,
};

use crate::paths::InstallPaths;

use super::manifest::entrypoint_target;
use super::transaction::{verify_release_target, PendingRelease, PENDING_RELEASE_SCHEMA_VERSION};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntrypointState {
    Missing,
    Regular,
    ManagedSymlink,
}

pub(in crate::actions::releases) fn plan_entrypoint_changes(
    paths: &InstallPaths,
    binaries: &[String],
    generation: &str,
) -> Result<PendingRelease> {
    // Planning inspects live entrypoints but never changes them
    let link_root = entrypoint_target();
    let previous_current = read_symlink(&paths.installed_current_link()?)?;
    if let Some(previous) = previous_current.as_ref() {
        verify_release_target(paths, previous)
            .context("verify current generation before retaining it for rollback")?;
    }
    let (legacy_entrypoints, created_entrypoints) =
        classify_entrypoint_changes(paths, binaries, &link_root)?;

    Ok(PendingRelease {
        schema_version: PENDING_RELEASE_SCHEMA_VERSION,
        generation: generation.to_string(),
        new_current: PathBuf::from("releases").join(generation),
        previous_current,
        legacy_entrypoints,
        created_entrypoints,
    })
}

pub(in crate::actions::releases) fn apply_entrypoint_changes(
    paths: &InstallPaths,
    pending: &PendingRelease,
) -> Result<()> {
    // The durable journal must exist before this function is called
    create_directory_all(&paths.bin_dir, 0o755).context("create binary entrypoint directory")?;
    let rollback_dir = rollback_bin_dir(paths, pending)?;
    create_directory_all(&rollback_dir, 0o700).context("create binary rollback directory")?;

    // Legacy files move to the generation-scoped rollback area before links appear
    for name in &pending.legacy_entrypoints {
        let entry = paths.bin_dir.join(name);
        let backup = rollback_dir.join(name);
        match rename_regular_file_no_replace(&entry, &backup)? {
            RenameRegularFileOutcome::Renamed => {}
            RenameRegularFileOutcome::SourceMissing => {
                return Err(anyhow!(
                    "binary entrypoint disappeared before migration: {name}"
                ));
            }
            RenameRegularFileOutcome::DestinationExists => {
                return Err(anyhow!(
                    "binary rollback entry already exists: {}",
                    backup.display()
                ));
            }
        }
    }

    let expected_root = entrypoint_target();
    // Every public binary name resolves through the one current-generation switch
    for name in pending
        .legacy_entrypoints
        .iter()
        .chain(&pending.created_entrypoints)
    {
        let entry = paths.bin_dir.join(name);
        let expected = expected_root.join(name);
        match create_symlink_if_missing(&entry, &expected)? {
            CreateSymlinkOutcome::Created | CreateSymlinkOutcome::Unchanged => {}
            CreateSymlinkOutcome::TargetMismatch(actual) => {
                return Err(anyhow!(
                    "binary entrypoint changed to unmanaged target {}",
                    actual.display()
                ));
            }
        }
    }
    Ok(())
}

pub(in crate::actions::releases) fn rollback_entrypoint_changes(
    paths: &InstallPaths,
    pending: &PendingRelease,
) -> Result<()> {
    let expected_root = entrypoint_target();
    let rollback_dir = rollback_bin_dir(paths, pending)?;

    // Journal state permits recovery before, during, or after each legacy move
    for name in &pending.legacy_entrypoints {
        rollback_legacy_entrypoint(
            &paths.bin_dir.join(name),
            &rollback_dir.join(name),
            &expected_root.join(name),
            name,
        )?;
    }
    // Newly created links contain no legacy bytes and can be removed directly
    for name in &pending.created_entrypoints {
        rollback_created_entrypoint(&paths.bin_dir.join(name), &expected_root.join(name), name)?;
    }

    let rollback_generation = paths.installed_rollback_root()?.join(&pending.generation);
    if rollback_generation.exists() {
        remove_directory_tree(&rollback_generation).context("remove completed rollback data")?;
    }
    Ok(())
}

fn classify_entrypoint_changes(
    paths: &InstallPaths,
    binaries: &[String],
    expected_root: &Path,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut legacy = Vec::new();
    let mut created = Vec::new();
    // Managed links need no per-entrypoint rollback record
    for name in binaries {
        match inspect_entrypoint(&paths.bin_dir.join(name), &expected_root.join(name))? {
            EntrypointState::Missing => created.push(name.clone()),
            EntrypointState::Regular => legacy.push(name.clone()),
            EntrypointState::ManagedSymlink => {}
        }
    }
    Ok((legacy, created))
}

fn rollback_legacy_entrypoint(
    entry: &Path,
    backup: &Path,
    expected: &Path,
    name: &str,
) -> Result<()> {
    let entry_state = inspect_entrypoint(entry, expected)?;
    let backup_state = inspect_backup(backup)?;
    match (entry_state, backup_state) {
        // The move never started or a prior recovery already restored it
        (EntrypointState::Regular, EntrypointState::Missing) => Ok(()),
        // The move completed but link creation did not
        (EntrypointState::Missing, EntrypointState::Regular) => restore_backup(backup, entry, name),
        // Both the move and managed-link publication completed
        (EntrypointState::ManagedSymlink, EntrypointState::Regular) => {
            remove_expected_entrypoint(entry, expected)?;
            restore_backup(backup, entry, name)
        }
        (EntrypointState::Regular, EntrypointState::Regular) => Err(anyhow!(
            "binary rollback has both live and backup files: {name}"
        )),
        (EntrypointState::Missing, EntrypointState::Missing) => {
            Err(anyhow!("binary rollback lost both copies: {name}"))
        }
        (EntrypointState::ManagedSymlink, EntrypointState::Missing) => Err(anyhow!(
            "binary rollback source is missing behind managed entrypoint: {name}"
        )),
        (_, EntrypointState::ManagedSymlink) => Err(anyhow!(
            "binary rollback copy is an unexpected symbolic link: {name}"
        )),
    }
}

fn rollback_created_entrypoint(entry: &Path, expected: &Path, name: &str) -> Result<()> {
    match inspect_entrypoint(entry, expected)? {
        // Link creation never started or a prior recovery already removed it
        EntrypointState::Missing => Ok(()),
        EntrypointState::ManagedSymlink => remove_expected_entrypoint(entry, expected),
        EntrypointState::Regular => Err(anyhow!(
            "new binary entrypoint changed to a regular file during rollback: {name}"
        )),
    }
}

fn inspect_entrypoint(path: &Path, expected: &Path) -> Result<EntrypointState> {
    // Link metadata keeps classification on the entrypoint itself
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(EntrypointState::Missing),
        Ok(metadata) if metadata.file_type().is_file() => Ok(EntrypointState::Regular),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let actual = fs::read_link(path)
                .with_context(|| format!("inspect binary entrypoint {}", path.display()))?;
            if actual == expected {
                Ok(EntrypointState::ManagedSymlink)
            } else {
                Err(anyhow!(
                    "binary entrypoint {} points to an unmanaged target {}",
                    path.display(),
                    actual.display()
                ))
            }
        }
        Ok(_metadata) => Err(anyhow!(
            "binary entrypoint is not a regular file or managed link: {}",
            path.display()
        )),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
    }
}

fn inspect_backup(path: &Path) -> Result<EntrypointState> {
    // Backups must remain regular files and never redirect recovery
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(EntrypointState::Missing),
        Ok(metadata) if metadata.file_type().is_file() => Ok(EntrypointState::Regular),
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(EntrypointState::ManagedSymlink),
        Ok(_metadata) => Err(anyhow!(
            "binary rollback copy is not a regular file: {}",
            path.display()
        )),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
    }
}

fn restore_backup(backup: &Path, entry: &Path, name: &str) -> Result<()> {
    match rename_regular_file_no_replace(backup, entry)? {
        RenameRegularFileOutcome::Renamed => Ok(()),
        RenameRegularFileOutcome::SourceMissing => {
            Err(anyhow!("binary rollback source disappeared: {name}"))
        }
        RenameRegularFileOutcome::DestinationExists => {
            Err(anyhow!("binary rollback destination changed: {name}"))
        }
    }
}

fn remove_expected_entrypoint(entry: &Path, expected: &Path) -> Result<()> {
    match remove_symlink_if_target(entry, expected)? {
        RemoveSymlinkOutcome::Removed | RemoveSymlinkOutcome::Missing => Ok(()),
        RemoveSymlinkOutcome::TargetMismatch(actual) => Err(anyhow!(
            "binary entrypoint changed during rollback to {}",
            actual.display()
        )),
    }
}

fn rollback_bin_dir(paths: &InstallPaths, pending: &PendingRelease) -> Result<PathBuf> {
    Ok(paths
        .installed_rollback_root()?
        .join(&pending.generation)
        .join("bin"))
}
