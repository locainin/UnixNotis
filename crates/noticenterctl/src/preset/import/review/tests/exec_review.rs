use super::super::exec_review::{
    confirm_import_exec_content, confirm_import_exec_content_with_terminal_state,
    render_exec_content_review_with_style, write_exec_content_review, ReviewStyle,
};
use crate::preset::import::review::checks::{
    ImportedExecCommand, ImportedExecContent, ImportedExecFile,
};
use std::path::PathBuf;

use crate::test_support::{test_env_lock, EnvGuard};

#[test]
fn exec_review_renders_commands_and_files() {
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd".to_string(),
                command: "scripts/check.sh".to_string(),
            }],
            files: vec![ImportedExecFile {
                relative_path: PathBuf::from("scripts/check.sh"),
                contents: b"#!/bin/sh\necho ok\n".to_vec(),
                mode: 0o755,
            }],
        },
        ReviewStyle { color: false },
    );

    assert!(review.contains("widgets.stats[0].cmd = scripts/check.sh"));
    assert!(review.contains("This preset contains executable commands or bundled scripts"));
    assert!(review.contains("Only continue if the source is trusted"));
    assert!(review.contains("Command entries"));
    assert!(review.contains("Bundled executable files"));
    assert!(review.contains("== scripts/check.sh (mode 755) =="));
    assert!(review.contains("#!/bin/sh"));
}

#[test]
fn exec_review_allows_empty_or_explicitly_trusted_exec_content() {
    let empty = ImportedExecContent {
        commands: Vec::new(),
        files: Vec::new(),
    };
    confirm_import_exec_content(&empty, false).expect("empty content does not need review");

    let content = imported_exec_content();
    confirm_import_exec_content(&content, true).expect("allow_exec bypasses interactive review");
}

#[test]
fn exec_review_rejects_untrusted_exec_content_when_not_interactive() {
    let content = imported_exec_content();

    let error = confirm_import_exec_content_with_terminal_state(&content, false, false)
        .expect_err("noninteractive untrusted exec content should fail closed");

    assert!(error
        .to_string()
        .contains("preset import found executable commands or bundled scripts"));
}

#[test]
fn exec_review_style_can_add_color() {
    let title = ReviewStyle { color: true }.title("review");
    assert!(title.contains("\u{1b}[1;36m"));
    assert!(title.ends_with("\u{1b}[0m"));
}

#[test]
fn exec_review_writer_ignores_pager_environment() {
    let _lock = test_env_lock();
    let _pager = EnvGuard::set("PAGER", "sh -c 'echo pwned'");

    let mut written = Vec::new();
    write_exec_content_review(&mut written, "review text\n").expect("write review");

    assert_eq!(written, b"review text\n");
}

fn imported_exec_content() -> ImportedExecContent {
    ImportedExecContent {
        commands: vec![ImportedExecCommand {
            slot: "widgets.stats[0].cmd".to_string(),
            command: "scripts/check.sh".to_string(),
        }],
        files: vec![ImportedExecFile {
            relative_path: PathBuf::from("scripts/check.sh"),
            contents: b"#!/bin/sh\necho ok\n".to_vec(),
            mode: 0o755,
        }],
    }
}
