//! Aggregate review output limit coverage

use super::super::measure::measured_review_size;
use super::super::model::ReviewDetail;
use super::{
    render_exec_content_review_with_style, ImportedExecCommand, ImportedExecContent,
    ImportedExecFile, PathBuf, ReviewStyle, MAX_COMPLETE_REVIEW_OUTPUT_BYTES,
};

#[test]
fn exec_review_uses_metadata_when_escaped_bodies_exceed_aggregate_limit() {
    // NUL is valid UTF-8 but expands from one byte to an eight-byte visible escape
    let files = (0..32)
        .map(|index| ImportedExecFile {
            relative_path: PathBuf::from(format!("assets/control-heavy-{index}.txt")),
            contents: vec![0; 32 * 1_024],
            mode: 0o644,
        })
        .collect();
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd".to_string(),
                command: "sh assets/control-heavy-0.txt".to_string(),
            }],
            files,
        },
        ReviewStyle { color: false },
    );

    assert!(!review.complete);
    assert!(review.rendered.len() <= MAX_COMPLETE_REVIEW_OUTPUT_BYTES);
    assert!(review.rendered.contains("Review status: incomplete"));
    assert!(review
        .rendered
        .contains("complete output exceeds 8388608 bytes"));
    assert!(review
        .rendered
        .contains("== assets/control-heavy-31.txt (mode 644"));
    assert!(review.rendered.contains("file body omitted"));
    assert!(!review.rendered.contains("\\u{0000}"));
    assert!(review.rendered.len() < 16 * 1_024);
}

#[test]
fn exec_review_uses_summary_when_metadata_alone_exceeds_aggregate_limit() {
    let commands = (0..9)
        .map(|index| ImportedExecCommand {
            slot: format!("{index}-{}", "s".repeat(1_024 * 1_024)),
            command: "true".to_string(),
        })
        .collect();
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands,
            files: Vec::new(),
        },
        ReviewStyle { color: false },
    );

    assert!(!review.complete);
    assert!(review.rendered.len() <= MAX_COMPLETE_REVIEW_OUTPUT_BYTES);
    assert!(review.rendered.contains("Review metadata is omitted"));
    assert!(review.rendered.contains("9 commands; 0 bundled files"));
    assert!(review.rendered.len() < 1_024);
}

#[test]
fn exec_review_accepts_output_at_the_exact_aggregate_limit() {
    let mut content = ImportedExecContent {
        commands: vec![ImportedExecCommand {
            slot: "widgets.stats[0].cmd".to_string(),
            command: "sh assets/exact-0.txt".to_string(),
        }],
        files: (0..128)
            .map(|index| ImportedExecFile {
                relative_path: PathBuf::from(format!("assets/exact-{index}.txt")),
                contents: if index == 127 {
                    Vec::new()
                } else {
                    vec![b'x'; 64 * 1_024]
                },
                mode: 0o644,
            })
            .collect(),
    };

    // Header digit changes can shift the target by a few bytes, so converge on the exact size
    for _ in 0..8 {
        let measured = measured_review_size(
            &content,
            ReviewStyle { color: false },
            ReviewDetail::Full,
            true,
        )
        .expect("bounded review size should fit usize");
        if measured == MAX_COMPLETE_REVIEW_OUTPUT_BYTES {
            break;
        }
        let final_body = &mut content.files[127].contents;
        if measured < MAX_COMPLETE_REVIEW_OUTPUT_BYTES {
            final_body.extend(std::iter::repeat_n(
                b'x',
                MAX_COMPLETE_REVIEW_OUTPUT_BYTES - measured,
            ));
        } else {
            final_body.truncate(
                final_body
                    .len()
                    .checked_sub(measured - MAX_COMPLETE_REVIEW_OUTPUT_BYTES)
                    .expect("exact-size adjustment should stay inside the final file"),
            );
        }
    }

    let measured = measured_review_size(
        &content,
        ReviewStyle { color: false },
        ReviewDetail::Full,
        true,
    )
    .expect("bounded review size should fit usize");
    assert_eq!(measured, MAX_COMPLETE_REVIEW_OUTPUT_BYTES);

    let review = render_exec_content_review_with_style(&content, ReviewStyle { color: false });
    assert!(review.complete);
    assert_eq!(review.rendered.len(), MAX_COMPLETE_REVIEW_OUTPUT_BYTES);
}
