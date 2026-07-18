//! Terminal style policy coverage

use super::{
    render_exec_content_review_with_style, ImportedExecCommand, ImportedExecContent, ReviewStyle,
};

#[test]
fn exec_review_style_can_add_color() {
    let review = render_exec_content_review_with_style(
        &ImportedExecContent {
            commands: vec![ImportedExecCommand {
                slot: "widgets.stats[0].cmd".to_string(),
                command: "true".to_string(),
            }],
            files: Vec::new(),
        },
        ReviewStyle { color: true },
    );

    assert!(review.rendered.starts_with("\u{1b}[1;36m"));
    assert!(review.rendered.contains("\u{1b}[0m"));
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
