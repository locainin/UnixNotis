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
fn exec_review_style_can_add_color() {
    let title = ReviewStyle { color: true }.title("review");
    assert!(title.contains("\u{1b}[1;36m"));
    assert!(title.ends_with("\u{1b}[0m"));
}

#[test]
fn exec_review_style_honors_every_color_precondition() {
    assert!(ReviewStyle::for_terminal_state(true, false, None, None).color);
    assert!(!ReviewStyle::for_terminal_state(false, false, None, None).color);
    assert!(!ReviewStyle::for_terminal_state(true, true, None, None).color);
    assert!(!ReviewStyle::for_terminal_state(true, false, Some("0"), None).color);
    assert!(!ReviewStyle::for_terminal_state(true, false, None, Some("dumb")).color);
    assert!(ReviewStyle::for_terminal_state(true, false, Some("1"), Some("xterm")).color);
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
fn exec_review_sanitizes_terminal_controls_and_bounds_script_text() {
    let mut contents = b"#!/bin/sh\nprintf '\\033[2Jspoof'\n".to_vec();
    contents.extend(std::iter::repeat_n(b'x', 8_192));
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd\u{001b}[2J".to_string(),
                command: "scripts/check.sh\u{202e}spoof".to_string(),
            }],
            files: vec![ImportedExecFile {
                relative_path: PathBuf::from("scripts/check\u{001b}[2J.sh"),
                contents,
                mode: 0o755,
            }],
        },
        ReviewStyle { color: false },
    );

    assert!(!review.contains('\u{001b}'));
    assert!(!review.contains('\u{202e}'));
    assert!(review.contains("spoof"));
    assert!(review.contains("..."));
    assert!(review.len() < 6_000);
}

#[test]
fn exec_review_reports_entries_omitted_by_display_limits() {
    let commands = (0..65)
        .map(|index| ImportedExecCommand {
            slot: format!("slot-{index}"),
            command: "true".to_string(),
        })
        .collect();
    let files = (0..33)
        .map(|index| ImportedExecFile {
            relative_path: PathBuf::from(format!("scripts/{index}")),
            contents: b"true\n".to_vec(),
            mode: 0o755,
        })
        .collect();

    let review = render_exec_content_review_with_style(
        &ImportedExecContent { commands, files },
        ReviewStyle { color: false },
    );

    assert!(review.contains("<1 additional command entries omitted>"));
    assert!(review.contains("<1 additional executable files omitted>"));
    assert!(!review.contains("slot-64"));
    assert!(!review.contains("scripts/32"));
}

#[test]
fn exec_review_does_not_report_omissions_at_exact_display_limits() {
    let commands = (0..64)
        .map(|index| ImportedExecCommand {
            slot: format!("slot-{index}"),
            command: "true".to_string(),
        })
        .collect();
    let files = (0..32)
        .map(|index| ImportedExecFile {
            relative_path: PathBuf::from(format!("scripts/{index}")),
            contents: b"true\n".to_vec(),
            mode: 0o755,
        })
        .collect();

    let review = render_exec_content_review_with_style(
        &ImportedExecContent { commands, files },
        ReviewStyle { color: false },
    );

    assert!(!review.contains("additional command entries omitted"));
    assert!(!review.contains("additional executable files omitted"));
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
