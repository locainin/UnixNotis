use super::super::exec_review::{
    confirm_import_exec_content, confirm_import_exec_content_with_interaction,
    confirm_import_exec_content_with_terminal_state, ensure_exec_review_complete,
    write_exec_content_review,
};
use super::super::render::RenderedExecReview;
use crate::preset::import::review::checks::{
    ImportedExecCommand, ImportedExecContent, ImportedExecFile,
};
use std::cell::Cell;
use std::path::PathBuf;

use crate::test_support::{test_env_lock, EnvGuard};

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
fn exec_review_rejects_each_independent_exec_content_kind() {
    let command_only = ImportedExecContent {
        commands: imported_exec_content().commands,
        files: Vec::new(),
    };
    let file_only = ImportedExecContent {
        commands: Vec::new(),
        files: imported_exec_content().files,
    };

    for content in [&command_only, &file_only] {
        confirm_import_exec_content_with_terminal_state(content, false, false)
            .expect_err("either executable content kind must require trust");
    }
}

#[test]
fn exec_review_public_entry_rejects_untrusted_content_without_a_terminal() {
    if std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
    {
        return;
    }

    confirm_import_exec_content(&imported_exec_content(), false)
        .expect_err("captured test process must reject an interactive review");
}

#[test]
fn exec_review_writer_ignores_pager_environment() {
    let _lock = test_env_lock();
    let _pager = EnvGuard::set("PAGER", "sh -c 'echo pwned'");

    let mut written = Vec::new();
    write_exec_content_review(&mut written, "review text\n").expect("write review");

    assert_eq!(written, b"review text\n");
}

#[test]
fn exec_review_incomplete_result_requires_explicit_override() {
    let review = RenderedExecReview {
        rendered: "Review status: incomplete\n".to_string(),
        complete: false,
    };

    let error = ensure_exec_review_complete(&review)
        .expect_err("partial review must require the explicit override");

    assert!(error.to_string().contains("--allow-exec"));
}

#[test]
fn exec_review_incomplete_flow_never_reaches_final_approval_prompt() {
    let content = ImportedExecContent {
        commands: vec![ImportedExecCommand {
            slot: "widgets.stats[0].cmd".to_string(),
            command: "sh assets/large.txt".to_string(),
        }],
        files: vec![ImportedExecFile {
            relative_path: PathBuf::from("assets/large.txt"),
            contents: vec![b'x'; 70 * 1_024],
            mode: 0o644,
        }],
    };
    let mut questions = Vec::new();
    let review_was_shown = Cell::new(false);

    let error = confirm_import_exec_content_with_interaction(
        &content,
        false,
        true,
        |question| {
            questions.push(question.to_string());
            Ok(true)
        },
        |review| {
            review_was_shown.set(true);
            assert!(!review.complete);
            Ok(())
        },
    )
    .expect_err("incomplete review must stop before approval");

    assert!(review_was_shown.get());
    assert_eq!(questions, ["Inspect executable content now?"]);
    assert!(error.to_string().contains("--allow-exec"));
}

#[test]
fn exec_review_complete_flow_requires_inspection_before_final_approval() {
    let mut questions = Vec::new();
    let review_was_shown = Cell::new(false);

    confirm_import_exec_content_with_interaction(
        &imported_exec_content(),
        false,
        true,
        |question| {
            questions.push(question.to_string());
            Ok(true)
        },
        |review| {
            review_was_shown.set(true);
            assert!(review.complete);
            Ok(())
        },
    )
    .expect("complete inspected review can reach final approval");

    assert!(review_was_shown.get());
    assert_eq!(
        questions,
        [
            "Inspect executable content now?",
            "Import this preset anyway?"
        ]
    );
}

#[test]
fn exec_review_declined_inspection_never_shows_or_approves_content() {
    let review_was_shown = Cell::new(false);
    let mut prompt_count = 0;

    let error = confirm_import_exec_content_with_interaction(
        &imported_exec_content(),
        false,
        true,
        |_question| {
            prompt_count += 1;
            Ok(false)
        },
        |_review| {
            review_was_shown.set(true);
            Ok(())
        },
    )
    .expect_err("declining required inspection must cancel import");

    assert_eq!(prompt_count, 1);
    assert!(!review_was_shown.get());
    assert!(error.to_string().contains("review is required"));
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
