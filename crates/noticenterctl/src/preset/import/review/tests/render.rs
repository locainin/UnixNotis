use super::super::render::{render_exec_content_review_with_style, ReviewStyle};
use crate::preset::import::review::checks::{
    ImportedExecCommand, ImportedExecContent, ImportedExecFile,
};
use std::path::PathBuf;

#[test]
fn exec_review_renders_complete_commands_files_and_metadata() {
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
    let rendered = &review.rendered;

    assert!(review.complete);
    assert!(rendered.contains("widgets.stats[0].cmd = scripts/check.sh"));
    assert!(rendered.contains("This preset contains executable commands or bundled scripts"));
    assert!(rendered.contains("Only continue if the source is trusted"));
    assert!(rendered.contains("Review status: complete"));
    assert!(rendered.contains("Command entries"));
    assert!(rendered.contains("Bundled files available to commands"));
    assert!(rendered.contains("== scripts/check.sh (mode 755, 18 bytes, BLAKE3 "));
    assert!(rendered.contains("#!/bin/sh"));
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
fn exec_review_escapes_terminal_controls_without_hiding_text() {
    let mut contents = b"#!/bin/sh\nprintf '\x1b[2Jspoof'\r\n\t\\path\n".to_vec();
    contents.extend(std::iter::repeat_n(b'x', 8_192));
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd\u{001b}[2J".to_string(),
                command: "scripts/check.sh\nnext\u{202e}spoof".to_string(),
            }],
            files: vec![ImportedExecFile {
                relative_path: PathBuf::from("scripts/check\u{001b}[2J.sh"),
                contents,
                mode: 0o755,
            }],
        },
        ReviewStyle { color: false },
    );
    let rendered = &review.rendered;

    assert!(review.complete);
    assert!(!rendered.contains('\u{001b}'));
    assert!(!rendered.contains('\u{202e}'));
    assert!(rendered.contains("\\u{001b}"));
    assert!(rendered.contains("\\u{202e}"));
    assert!(rendered.contains("scripts/check.sh\\nnext\\u{202e}spoof"));
    assert!(rendered.contains("#!/bin/sh\nprintf"));
    assert!(rendered.contains("printf '\\u{001b}[2Jspoof'\\r\n\\t\\\\path\n"));
    assert!(rendered.contains("spoof"));
    assert!(rendered.ends_with('\n'));
    assert!(rendered.len() > 8_192);
}

#[test]
fn exec_review_shows_every_command_and_file_without_count_omissions() {
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

    assert!(review.complete);
    assert!(review.rendered.contains("slot-64 = true"));
    assert!(review.rendered.contains("== scripts/32 (mode 755"));
    assert!(!review
        .rendered
        .contains("additional command entries omitted"));
    assert!(!review
        .rendered
        .contains("additional executable files omitted"));
}

#[test]
fn exec_review_shows_long_command_and_script_suffixes_in_full() {
    let command = format!("{}sh assets/payload.dat", ":;".repeat(1_000));
    let script_tail = "printf '%s\\n' complete-tail\n";
    let script = format!("#!/bin/sh\n{}{}", "# padding\n".repeat(560), script_tail);
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd".to_string(),
                command: command.clone(),
            }],
            files: vec![ImportedExecFile {
                relative_path: PathBuf::from("scripts/long-helper"),
                contents: script.into_bytes(),
                mode: 0o755,
            }],
        },
        ReviewStyle { color: false },
    );

    assert!(review.complete);
    assert!(review.rendered.contains(&command));
    assert!(review.rendered.contains("complete-tail"));
    assert!(!review.rendered.contains("..."));
}

#[test]
fn exec_review_marks_oversized_text_as_incomplete() {
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd".to_string(),
                command: "sh assets/large.txt".to_string(),
            }],
            files: vec![ImportedExecFile {
                relative_path: PathBuf::from("assets/large.txt"),
                contents: vec![b'x'; 70 * 1_024],
                mode: 0o644,
            }],
        },
        ReviewStyle { color: false },
    );

    assert!(!review.complete);
    assert!(review.rendered.contains("Review status: incomplete"));
    assert!(review.rendered.contains("text not displayed"));
    assert!(review.rendered.contains("BLAKE3"));
}

#[test]
fn exec_review_marks_oversized_command_as_incomplete() {
    let long_command = "x".repeat(70 * 1_024);
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd".to_string(),
                command: long_command.clone(),
            }],
            files: Vec::new(),
        },
        ReviewStyle { color: false },
    );

    assert!(!review.complete);
    assert!(review.rendered.contains("command not displayed"));
    assert!(!review.rendered.contains(&long_command));
}

#[test]
fn exec_review_represents_binary_files_with_size_and_digest() {
    let contents = vec![0, 159, 146, 150];
    let digest = blake3::hash(&contents).to_hex().to_string();
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd".to_string(),
                command: "loader assets/module.so".to_string(),
            }],
            files: vec![ImportedExecFile {
                relative_path: PathBuf::from("assets/module.so"),
                contents,
                mode: 0o644,
            }],
        },
        ReviewStyle { color: false },
    );

    assert!(review.complete);
    assert!(review.rendered.contains("4 bytes"));
    assert!(review.rendered.contains(&digest));
    assert!(review
        .rendered
        .contains("binary content represented by metadata above"));
}
