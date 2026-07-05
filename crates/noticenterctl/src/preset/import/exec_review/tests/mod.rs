use super::{
    finish_pager, pager_command_parts, pager_enables_raw_control,
    render_exec_content_review_with_style, ReviewStyle,
};
use crate::preset::import::checks::{ImportedExecCommand, ImportedExecContent, ImportedExecFile};
use std::env;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

// Pager tests mutate one process-global env var, so they need one tiny lock
static PAGER_ENV_LOCK: Mutex<()> = Mutex::new(());

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
    assert!(review.contains("== scripts/check.sh (mode 755) =="));
    assert!(review.contains("#!/bin/sh"));
}

#[test]
fn exec_review_style_can_add_color() {
    let title = ReviewStyle { color: true }.title("review");
    assert!(title.contains("\u{1b}[1;36m"));
    assert!(title.ends_with("\u{1b}[0m"));
}

#[test]
fn pager_command_adds_raw_control_for_less() {
    let _guard = PAGER_ENV_LOCK.lock().expect("lock pager env");
    let original = env::var_os("PAGER");
    unsafe {
        env::set_var("PAGER", "less -F");
    }

    let pager = pager_command_parts().expect("build pager");

    match original {
        Some(value) => unsafe {
            env::set_var("PAGER", value);
        },
        None => unsafe {
            env::remove_var("PAGER");
        },
    }

    assert_eq!(pager, vec!["less", "-F", "-R"]);
}

#[test]
fn pager_command_keeps_existing_raw_control_flag() {
    assert!(pager_enables_raw_control(&[
        "less".to_string(),
        "-FR".to_string()
    ]));
    assert!(pager_enables_raw_control(&[
        "less".to_string(),
        "-R".to_string()
    ]));
    assert!(!pager_enables_raw_control(&[
        "less".to_string(),
        "-F".to_string()
    ]));
}

#[test]
fn pager_command_respects_quoted_arguments() {
    let _guard = PAGER_ENV_LOCK.lock().expect("lock pager env");
    let original = env::var_os("PAGER");
    unsafe {
        env::set_var("PAGER", "less --prompt='unixnotis review'");
    }

    let pager = pager_command_parts().expect("build pager");

    match original {
        Some(value) => unsafe {
            env::set_var("PAGER", value);
        },
        None => unsafe {
            env::remove_var("PAGER");
        },
    }

    assert_eq!(
        pager,
        vec![
            "less".to_string(),
            "--prompt=unixnotis review".to_string(),
            "-R".to_string()
        ]
    );
}

#[test]
fn finish_pager_reaps_child_after_stdin_failure() {
    let child = Command::new("sh")
        .arg("-c")
        .arg("exit 0")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("spawn pager");

    let error = finish_pager(
        child,
        &["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe")),
    )
    .expect_err("stdin failure should surface");

    assert!(error
        .to_string()
        .contains("write executable content review to pager"));
}
