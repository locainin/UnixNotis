use super::super::sanitize::JOURNAL_LINE_LIMIT;
use super::super::systemd::*;
use crate::doctor::report::{DoctorLogResult, DoctorLogSource};
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn systemd_without_verbose_mode_does_not_attempt_journal_collection() {
    let result = collect_systemd_logs(false).await;

    assert!(matches!(
        result,
        DoctorLogResult::Unavailable {
            source: DoctorLogSource::SystemdJournal,
            reason,
            ..
        } if reason.contains("--verbose")
    ));
}

#[tokio::test]
async fn oversized_journal_output_returns_a_bounded_report() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-bounded-journal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create fake tool directory");
    let journalctl = root.join("journalctl");
    std::fs::write(
        &journalctl,
        "#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 4000 ]; do\n  printf 'bounded journal line %s\\n' \"$i\"\n  i=$((i + 1))\ndone\nsleep 10\n",
    )
    .expect("write fake journal command");
    std::fs::set_permissions(&journalctl, std::fs::Permissions::from_mode(0o755))
        .expect("make fake journal executable");
    let _tools = crate::system_tools::routing::use_fake_tool_bin(&root);

    let collection = read_recent_journal("unixnotis-daemon.service")
        .await
        .expect("oversized journal output should be truncated");

    assert_eq!(collection.lines.len(), JOURNAL_LINE_LIMIT);
    assert!(collection.byte_truncated);
    assert!(collection.was_truncated());
    assert!(collection
        .lines
        .iter()
        .all(|line| line.starts_with("bounded journal")));
    std::fs::remove_dir_all(root).expect("remove fake tool directory");
}

#[tokio::test]
async fn failed_journal_command_is_not_reported_as_collected_output() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-failed-journal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create fake tool directory");
    let journalctl = root.join("journalctl");
    std::fs::write(&journalctl, "#!/bin/sh\nprintf 'partial line\\n'\nexit 1\n")
        .expect("write failing journal command");
    std::fs::set_permissions(&journalctl, std::fs::Permissions::from_mode(0o755))
        .expect("make fake journal executable");
    let _tools = crate::system_tools::routing::use_fake_tool_bin(&root);

    let error = read_recent_journal("unixnotis-daemon.service")
        .await
        .expect_err("non-zero journal command must remain unavailable");

    assert!(error.contains("journalctl exited with status"));
    std::fs::remove_dir_all(root).expect("remove fake tool directory");
}
