//! Durable multi-file restore publication and recovery

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use unixnotis_core::filesystem::{
    create_directory_all, open_regular_file, read_regular_file_bounded, remove_directory_tree,
    remove_regular_file, write_file_atomic, write_file_if_missing, CreateDirectoryOutcome,
};

use super::restore::MAX_RESTORE_FILE_BYTES;

const RESTORE_JOURNAL_FILE: &str = ".unixnotis-restore-pending.json";
const RESTORE_TRANSACTION_PREFIX: &str = ".unixnotis-restore-";
const RESTORE_JOURNAL_SCHEMA: u32 = 1;
const MAX_RESTORE_JOURNAL_BYTES: u64 = 256 * 1024;
const TRANSACTION_DIRECTORY_ATTEMPTS: u8 = 16;
static RESTORE_TRANSACTION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) struct RestoreWrite<'a> {
    pub(super) label: &'a str,
    pub(super) target: &'a Path,
    pub(super) mode: u32,
    pub(super) contents: &'a [u8],
}

#[derive(Debug, Deserialize, Serialize)]
struct RestoreJournal {
    schema_version: u32,
    transaction_dir: String,
    entries: Vec<RestoreJournalEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RestoreJournalEntry {
    target: PathBuf,
    staged: PathBuf,
    staged_size: u64,
    previous: PreviousFile,
}

#[derive(Debug, Deserialize, Serialize)]
enum PreviousFile {
    Missing,
    Existing {
        rollback: PathBuf,
        size: u64,
        mode: u32,
    },
}

pub(super) fn apply_restore_transaction(
    config_dir: &Path,
    writes: &[RestoreWrite<'_>],
    post_validate: impl FnOnce() -> Result<()>,
) -> Result<()> {
    apply_restore_transaction_with_writer(config_dir, writes, post_validate, write_file_atomic)
}

fn apply_restore_transaction_with_writer(
    config_dir: &Path,
    writes: &[RestoreWrite<'_>],
    post_validate: impl FnOnce() -> Result<()>,
    mut publish: impl FnMut(&Path, &[u8], u32) -> std::io::Result<()>,
) -> Result<()> {
    // One journal owns the config tree so separate restores cannot overlap
    if pending_journal(config_dir)?.is_some() {
        return Err(anyhow!(
            "an incomplete restore transaction must be recovered before another restore"
        ));
    }
    let journal = prepare_restore_transaction(config_dir, writes)?;
    let transaction_dir = config_dir.join(&journal.transaction_dir);

    // Every payload comes from the bounded staged copy recorded in the journal
    let operation = (|| {
        for (write, entry) in writes.iter().zip(&journal.entries) {
            let staged = read_exact_transaction_file(
                &transaction_dir.join(&entry.staged),
                entry.staged_size,
            )?;
            publish(write.target, &staged, write.mode)
                .with_context(|| format!("failed to restore {}", write.label))?;
        }
        post_validate()
    })();
    if let Err(error) = operation {
        // Failed publication keeps recovery authority until rollback is complete
        return Err(rollback_or_retain(config_dir, &journal, error));
    }

    // Journal removal is the transaction commit point
    finish_transaction(config_dir, &journal)?;
    Ok(())
}

pub(super) fn recover_pending_restore(config_dir: &Path) -> Result<bool> {
    let Some(journal) = pending_journal(config_dir)? else {
        return Ok(false);
    };
    // Recovery trusts only a fully validated local journal
    validate_journal(&journal)?;
    rollback_transaction(config_dir, &journal)
        .context("recover interrupted config restore transaction")?;
    finish_transaction(config_dir, &journal)?;
    Ok(true)
}

fn prepare_restore_transaction(
    config_dir: &Path,
    writes: &[RestoreWrite<'_>],
) -> Result<RestoreJournal> {
    create_directory_all(config_dir, 0o700).context("create config directory for restore")?;
    let transaction_dir = reserve_transaction_directory(config_dir)?;
    let prepared = (|| {
        // Staged and rollback data stay private to this transaction
        create_directory_all(&transaction_dir.join("staged"), 0o700)
            .context("create restore staging directory")?;
        create_directory_all(&transaction_dir.join("rollback"), 0o700)
            .context("create restore rollback directory")?;

        let mut entries = Vec::with_capacity(writes.len());
        let mut targets = HashSet::new();
        for (index, write) in writes.iter().enumerate() {
            // Targets are stored relative to the pinned config root
            let target = relative_target(config_dir, write.target)?;
            if !targets.insert(target.clone()) {
                return Err(anyhow!(
                    "restore transaction contains duplicate live targets"
                ));
            }
            let staged = PathBuf::from("staged").join(index.to_string());
            // Payload staging happens before any live target can change
            write_file_atomic(&transaction_dir.join(&staged), write.contents, write.mode)
                .with_context(|| format!("stage restore payload for {}", write.label))?;
            let previous =
                snapshot_previous_file(write.target, &transaction_dir, index, write.label)?;
            entries.push(RestoreJournalEntry {
                target,
                staged,
                staged_size: u64::try_from(write.contents.len()).unwrap_or(u64::MAX),
                previous,
            });
        }
        let journal = RestoreJournal {
            schema_version: RESTORE_JOURNAL_SCHEMA,
            transaction_dir: transaction_dir
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("restore transaction directory name is not UTF-8"))?
                .to_string(),
            entries,
        };
        validate_journal(&journal)?;
        let bytes = serde_json::to_vec_pretty(&journal).context("serialize restore journal")?;
        if !journal_size_is_allowed(u64::try_from(bytes.len()).unwrap_or(u64::MAX)) {
            return Err(anyhow!("restore journal exceeds its safe byte limit"));
        }
        if !write_file_if_missing(&config_dir.join(RESTORE_JOURNAL_FILE), &bytes, 0o600)
            .context("publish restore transaction journal")?
        {
            return Err(anyhow!("restore transaction journal already exists"));
        }
        Ok(journal)
    })();
    if prepared.is_err() {
        let _cleanup = remove_directory_tree(&transaction_dir);
    }
    prepared
}

fn snapshot_previous_file(
    target: &Path,
    transaction_dir: &Path,
    index: usize,
    label: &str,
) -> Result<PreviousFile> {
    let file = match open_regular_file(target) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PreviousFile::Missing)
        }
        Err(error) => {
            return match fs::symlink_metadata(target) {
                Ok(_metadata) => Err(anyhow!("restore target is not a regular file: {label}")),
                Err(metadata_error) => Err(error).context(format!(
                    "inspect restore target for {label}: {metadata_error}"
                )),
            }
        }
    };
    // One retained descriptor keeps rollback mode, length, and bytes on the same object
    let metadata = file
        .metadata()
        .with_context(|| format!("inspect live restore target for {label}"))?;
    if metadata.len() > MAX_RESTORE_FILE_BYTES {
        return Err(anyhow!(
            "live restore target exceeds its safe byte limit: {label}"
        ));
    }
    let mut contents = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(usize::MAX));
    file.take(MAX_RESTORE_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut contents)
        .with_context(|| format!("snapshot live restore target for {label}"))?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > MAX_RESTORE_FILE_BYTES {
        return Err(anyhow!(
            "live restore target grew beyond its safe byte limit: {label}"
        ));
    }
    let rollback = PathBuf::from("rollback").join(index.to_string());
    let mode = std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o777;
    write_file_atomic(&transaction_dir.join(&rollback), &contents, mode)
        .with_context(|| format!("stage restore rollback for {label}"))?;
    Ok(PreviousFile::Existing {
        rollback,
        size: u64::try_from(contents.len()).unwrap_or(u64::MAX),
        mode,
    })
}

fn rollback_transaction(config_dir: &Path, journal: &RestoreJournal) -> Result<()> {
    let transaction_dir = config_dir.join(&journal.transaction_dir);
    let mut errors = Vec::new();
    // Reverse order mirrors publication and limits partial dependency exposure
    for entry in journal.entries.iter().rev() {
        let target = config_dir.join(&entry.target);
        let result = match &entry.previous {
            PreviousFile::Missing => remove_regular_file(&target)
                .map(|_removed| ())
                .map_err(anyhow::Error::from),
            PreviousFile::Existing {
                rollback,
                size,
                mode,
            } => read_exact_transaction_file(&transaction_dir.join(rollback), *size).and_then(
                |contents| {
                    write_file_atomic(&target, &contents, *mode).map_err(anyhow::Error::from)
                },
            ),
        };
        if let Err(error) = result {
            // Every remaining target is attempted before reporting incomplete recovery
            errors.push(format!("{}: {error}", target.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "restore rollback was incomplete: {}",
            errors.join("; ")
        ))
    }
}

fn rollback_or_retain(
    config_dir: &Path,
    journal: &RestoreJournal,
    operation_error: anyhow::Error,
) -> anyhow::Error {
    match rollback_transaction(config_dir, journal) {
        Ok(()) => match finish_transaction(config_dir, journal) {
            Ok(()) => operation_error,
            Err(cleanup_error) => operation_error.context(format!(
                "restore rollback completed but journal cleanup failed: {cleanup_error:#}"
            )),
        },
        Err(rollback_error) => operation_error.context(format!(
            "restore rollback was retained for recovery: {rollback_error:#}"
        )),
    }
}

fn finish_transaction(config_dir: &Path, journal: &RestoreJournal) -> Result<()> {
    remove_regular_file(&config_dir.join(RESTORE_JOURNAL_FILE))
        .context("remove committed restore journal")?;
    // The journal is the authority, so scratch cleanup becomes harmless after its removal
    let _cleanup = remove_directory_tree(&config_dir.join(&journal.transaction_dir));
    Ok(())
}

fn pending_journal(config_dir: &Path) -> Result<Option<RestoreJournal>> {
    let path = config_dir.join(RESTORE_JOURNAL_FILE);
    // Raw journal bytes are bounded before JSON allocation
    let bytes = match read_regular_file_bounded(&path, MAX_RESTORE_JOURNAL_BYTES) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("read pending restore journal"),
    };
    let journal = serde_json::from_slice(&bytes).context("parse pending restore journal")?;
    validate_journal(&journal)?;
    Ok(Some(journal))
}

fn validate_journal(journal: &RestoreJournal) -> Result<()> {
    // Unknown schemas never gain filesystem authority
    if journal.schema_version != RESTORE_JOURNAL_SCHEMA {
        return Err(anyhow!(
            "unsupported restore journal schema {}",
            journal.schema_version
        ));
    }
    validate_transaction_directory_name(&journal.transaction_dir)?;
    let mut targets = HashSet::new();
    for entry in &journal.entries {
        // Journal paths allow normal relative components only
        validate_relative_path(&entry.target)?;
        validate_relative_path(&entry.staged)?;
        if !matches!(entry.staged.components().next(), Some(Component::Normal(root)) if root == "staged")
        {
            return Err(anyhow!("restore journal contains an invalid staged path"));
        }
        if !targets.insert(entry.target.clone()) {
            return Err(anyhow!("restore journal contains duplicate live targets"));
        }
        if let PreviousFile::Existing { rollback, .. } = &entry.previous {
            validate_relative_path(rollback)?;
            if !matches!(rollback.components().next(), Some(Component::Normal(root)) if root == "rollback")
            {
                return Err(anyhow!("restore journal contains an invalid rollback path"));
            }
        }
    }
    Ok(())
}

fn relative_target(config_dir: &Path, target: &Path) -> Result<PathBuf> {
    let relative = target.strip_prefix(config_dir).map_err(|_error| {
        anyhow!(
            "restore target escapes the live config directory: {}",
            target.display()
        )
    })?;
    validate_relative_path(relative)?;
    Ok(relative.to_path_buf())
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(anyhow!("restore journal contains an unsafe relative path"));
    }
    Ok(())
}

fn validate_transaction_directory_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    if !name.starts_with(RESTORE_TRANSACTION_PREFIX)
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(anyhow!(
            "restore journal contains an unsafe transaction directory"
        ));
    }
    Ok(())
}

const fn journal_size_is_allowed(size: u64) -> bool {
    size <= MAX_RESTORE_JOURNAL_BYTES
}

const fn transaction_file_size_is_allowed(size: u64) -> bool {
    size <= MAX_RESTORE_FILE_BYTES
}

fn reserve_transaction_directory(config_dir: &Path) -> Result<PathBuf> {
    // Process, time, counter, and bounded retry values avoid attacker-selected names
    for attempt in 0..TRANSACTION_DIRECTORY_ATTEMPTS {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let counter = RESTORE_TRANSACTION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = config_dir.join(format!(
            "{RESTORE_TRANSACTION_PREFIX}{}-{nanos}-{counter}-{attempt}",
            std::process::id()
        ));
        if create_directory_all(&path, 0o700)? == CreateDirectoryOutcome::TargetCreated {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "unable to reserve a unique restore transaction directory"
    ))
}

fn read_exact_transaction_file(path: &Path, expected_size: u64) -> Result<Vec<u8>> {
    // Journal lengths remain bounded before reading staged or rollback content
    if !transaction_file_size_is_allowed(expected_size) {
        return Err(anyhow!(
            "restore transaction file exceeds its safe byte limit"
        ));
    }
    let contents = read_regular_file_bounded(path, expected_size)
        .with_context(|| format!("read restore transaction file {}", path.display()))?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) != expected_size {
        return Err(anyhow!("restore transaction file size changed"));
    }
    Ok(contents)
}

#[cfg(test)]
#[path = "tests/restore_transaction.rs"]
mod tests;
