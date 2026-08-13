use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::super::restore::MAX_RESTORE_FILE_BYTES;
use super::{
    apply_restore_transaction, apply_restore_transaction_with_writer, journal_size_is_allowed,
    pending_journal, prepare_restore_transaction, read_exact_transaction_file,
    recover_pending_restore, snapshot_previous_file, transaction_file_size_is_allowed,
    validate_journal, validate_relative_path, validate_transaction_directory_name, PreviousFile,
    RestoreJournal, RestoreJournalEntry, RestoreWrite, MAX_RESTORE_JOURNAL_BYTES,
};

#[test]
fn restore_transaction_rolls_back_every_published_file_after_a_late_failure() {
    let root = crate::test_support::fs::unique_temp_path("restore-transaction-rollback");
    fs::create_dir_all(&root).expect("create restore transaction fixture");
    let first = root.join("first.css");
    let second = root.join("second.css");
    fs::write(&first, "old first").expect("write first live file");
    fs::write(&second, "old second").expect("write second live file");
    fs::set_permissions(&first, fs::Permissions::from_mode(0o640)).expect("set first live mode");
    let writes = [
        RestoreWrite {
            label: "first.css",
            target: &first,
            mode: 0o644,
            contents: b"new first",
        },
        RestoreWrite {
            label: "second.css",
            target: &second,
            mode: 0o644,
            contents: b"new second",
        },
    ];
    let mut calls = 0usize;

    let error = apply_restore_transaction_with_writer(
        &root,
        &writes,
        || Ok(()),
        |target, contents, mode| {
            calls = calls.saturating_add(1);
            if calls == 2 {
                return Err(io::Error::other("injected second publish failure"));
            }
            unixnotis_core::filesystem::write_file_atomic(target, contents, mode)
        },
    )
    .expect_err("a late publish failure must fail the complete restore");

    assert!(error.to_string().contains("failed to restore second.css"));
    assert_eq!(
        fs::read_to_string(&first).expect("read restored first"),
        "old first"
    );
    assert_eq!(
        fs::read_to_string(&second).expect("read restored second"),
        "old second"
    );
    assert_eq!(
        fs::metadata(&first)
            .expect("inspect restored first")
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    assert!(pending_journal(&root)
        .expect("inspect pending journal")
        .is_none());
    fs::remove_dir_all(root).expect("remove restore transaction fixture");
}

#[test]
fn failed_restore_removes_a_new_file_published_before_the_failure() {
    let root = crate::test_support::fs::unique_temp_path("restore-created-rollback");
    fs::create_dir_all(&root).expect("create new-file rollback fixture");
    let created = root.join("created.css");
    let blocker = root.join("blocker.css");
    fs::write(&blocker, "old blocker").expect("write blocker file");
    let writes = [
        RestoreWrite {
            label: "created.css",
            target: &created,
            mode: 0o644,
            contents: b"new created",
        },
        RestoreWrite {
            label: "blocker.css",
            target: &blocker,
            mode: 0o644,
            contents: b"new blocker",
        },
    ];
    let mut calls = 0usize;

    apply_restore_transaction_with_writer(
        &root,
        &writes,
        || Ok(()),
        |target, contents, mode| {
            calls = calls.saturating_add(1);
            if calls == 2 {
                return Err(io::Error::other("injected blocker failure"));
            }
            unixnotis_core::filesystem::write_file_atomic(target, contents, mode)
        },
    )
    .expect_err("failed restore must remove a newly published target");

    assert!(!created.exists());
    assert_eq!(
        fs::read_to_string(blocker).expect("read blocker"),
        "old blocker"
    );
    fs::remove_dir_all(root).expect("remove new-file rollback fixture");
}

#[test]
fn restore_snapshot_rejects_special_targets_and_nonmissing_lookup_errors() {
    let root = crate::test_support::fs::unique_temp_path("restore-snapshot-errors");
    fs::create_dir_all(&root).expect("create restore snapshot error fixture");
    let directory = root.join("directory.css");
    fs::create_dir(&directory).expect("create directory target");
    let directory_write = [RestoreWrite {
        label: "directory.css",
        target: &directory,
        mode: 0o644,
        contents: b"new",
    }];
    let directory_error = apply_restore_transaction(&root, &directory_write, || Ok(()))
        .expect_err("directory target must fail before publication");
    assert!(directory_error
        .to_string()
        .contains("restore target is not a regular file"));

    let regular_parent = root.join("regular-parent");
    fs::write(&regular_parent, "not a directory").expect("write invalid parent");
    let invalid_target = regular_parent.join("child");
    let invalid_write = [RestoreWrite {
        label: "child",
        target: &invalid_target,
        mode: 0o644,
        contents: b"new",
    }];
    assert!(apply_restore_transaction(&root, &invalid_write, || Ok(())).is_err());
    assert!(pending_journal(&root)
        .expect("inspect failed journal")
        .is_none());
    fs::remove_dir_all(root).expect("remove restore snapshot error fixture");
}

#[test]
fn pending_restore_probe_propagates_nonmissing_journal_errors() {
    let root = crate::test_support::fs::unique_temp_path("restore-journal-probe-error");
    fs::create_dir_all(&root).expect("create restore journal error fixture");
    fs::create_dir(root.join(".unixnotis-restore-pending.json"))
        .expect("create invalid journal directory");

    assert!(recover_pending_restore(&root).is_err());
    fs::remove_dir_all(root).expect("remove restore journal error fixture");
}

#[test]
fn restore_transaction_byte_domains_accept_the_exact_limit_only() {
    assert!(journal_size_is_allowed(MAX_RESTORE_JOURNAL_BYTES));
    assert!(!journal_size_is_allowed(
        MAX_RESTORE_JOURNAL_BYTES.saturating_add(1)
    ));
    assert!(transaction_file_size_is_allowed(MAX_RESTORE_FILE_BYTES));
    assert!(!transaction_file_size_is_allowed(
        MAX_RESTORE_FILE_BYTES.saturating_add(1)
    ));
}

#[test]
fn restore_snapshot_accepts_a_live_file_at_the_exact_byte_limit() {
    let root = crate::test_support::fs::unique_temp_path("restore-snapshot-exact-limit");
    let transaction = root.join("transaction");
    fs::create_dir_all(transaction.join("rollback")).expect("create rollback directory");
    let target = root.join("config.toml");
    let file = fs::File::create(&target).expect("create exact-limit live file");
    file.set_len(MAX_RESTORE_FILE_BYTES)
        .expect("size exact-limit live file");

    let previous = snapshot_previous_file(&target, &transaction, 0, "config.toml")
        .expect("snapshot exact-limit live file");

    assert!(matches!(
        previous,
        PreviousFile::Existing {
            size: MAX_RESTORE_FILE_BYTES,
            ..
        }
    ));
    fs::remove_dir_all(root).expect("remove exact-limit snapshot fixture");
}

#[test]
fn restore_journal_validation_rejects_unsafe_paths_names_schemas_and_duplicates() {
    assert!(validate_relative_path(Path::new("theme/panel.css")).is_ok());
    assert!(validate_relative_path(Path::new("")).is_err());
    assert!(validate_relative_path(Path::new("../outside")).is_err());
    assert!(validate_transaction_directory_name(".unixnotis-restore-safe").is_ok());
    assert!(validate_transaction_directory_name("wrong-prefix").is_err());
    assert!(validate_transaction_directory_name(".unixnotis-restore-bad/child").is_err());

    let entry = RestoreJournalEntry {
        target: PathBuf::from("config.toml"),
        staged: PathBuf::from("staged/0"),
        staged_size: 0,
        previous: PreviousFile::Missing,
    };
    let mut journal = RestoreJournal {
        schema_version: 1,
        transaction_dir: ".unixnotis-restore-safe".to_string(),
        entries: vec![entry],
    };
    assert!(validate_journal(&journal).is_ok());
    journal.schema_version = 2;
    assert!(validate_journal(&journal).is_err());
    journal.schema_version = 1;
    journal.entries[0].staged = PathBuf::from("rollback/0");
    assert!(validate_journal(&journal).is_err());
    journal.entries[0].staged = PathBuf::from("staged/0");
    journal.entries.push(RestoreJournalEntry {
        target: PathBuf::from("config.toml"),
        staged: PathBuf::from("staged/1"),
        staged_size: 0,
        previous: PreviousFile::Existing {
            rollback: PathBuf::from("wrong/1"),
            size: 0,
            mode: 0o644,
        },
    });
    assert!(validate_journal(&journal).is_err());
    journal.entries[1].target = PathBuf::from("other.css");
    assert!(validate_journal(&journal).is_err());
}

#[test]
fn interrupted_restore_journal_restores_original_files_on_recovery() {
    let root = crate::test_support::fs::unique_temp_path("restore-transaction-recovery");
    fs::create_dir_all(&root).expect("create restore recovery fixture");
    let target = root.join("config.toml");
    fs::write(&target, "old config").expect("write old config");
    let writes = [RestoreWrite {
        label: "config.toml",
        target: &target,
        mode: 0o644,
        contents: b"new config",
    }];

    let journal = prepare_restore_transaction(&root, &writes).expect("prepare restore journal");
    let staged = read_exact_transaction_file(
        &root
            .join(&journal.transaction_dir)
            .join(&journal.entries[0].staged),
        journal.entries[0].staged_size,
    )
    .expect("read staged config");
    unixnotis_core::filesystem::write_file_atomic(&target, &staged, 0o644)
        .expect("simulate published config before process exit");
    assert_eq!(
        fs::read_to_string(&target).expect("read interrupted config"),
        "new config"
    );

    assert!(recover_pending_restore(&root).expect("recover interrupted restore"));
    assert_eq!(
        fs::read_to_string(&target).expect("read recovered config"),
        "old config"
    );
    assert!(pending_journal(&root)
        .expect("inspect recovered journal")
        .is_none());
    fs::remove_dir_all(root).expect("remove restore recovery fixture");
}

#[test]
fn successful_restore_commits_all_files_and_removes_its_journal() {
    let root = crate::test_support::fs::unique_temp_path("restore-transaction-success");
    fs::create_dir_all(&root).expect("create successful restore fixture");
    let existing = root.join("existing.css");
    let created = root.join("created.css");
    fs::write(&existing, "old").expect("write existing file");
    let writes = [
        RestoreWrite {
            label: "existing.css",
            target: &existing,
            mode: 0o644,
            contents: b"new existing",
        },
        RestoreWrite {
            label: "created.css",
            target: &created,
            mode: 0o600,
            contents: b"new created",
        },
    ];

    apply_restore_transaction(&root, &writes, || Ok(())).expect("commit restore transaction");

    assert_eq!(
        fs::read_to_string(existing).expect("read existing file"),
        "new existing"
    );
    assert_eq!(
        fs::read_to_string(created).expect("read created file"),
        "new created"
    );
    assert!(pending_journal(&root)
        .expect("inspect committed journal")
        .is_none());
    fs::remove_dir_all(root).expect("remove successful restore fixture");
}
