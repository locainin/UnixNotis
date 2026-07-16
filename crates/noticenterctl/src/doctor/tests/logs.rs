use super::*;
use std::os::unix::fs::PermissionsExt;
use unixnotis_core::service_manager::ServiceManagerKind;

#[test]
fn journal_limits_match_the_public_diagnostics_contract() {
    assert_eq!(JOURNAL_LINE_LIMIT, 30);
    assert_eq!(JOURNAL_TOTAL_BYTE_LIMIT, 32_768);
    assert_eq!(JOURNAL_LINE_CHAR_LIMIT, 512);
    assert!(!journal_output_exceeds_limit(32_768));
    assert!(journal_output_exceeds_limit(32_769));
}

#[test]
fn journal_output_is_sanitized_truncated_and_line_bounded() {
    let home = std::env::var("HOME").expect("HOME");
    let mut raw = String::new();
    for _ in 0..(JOURNAL_LINE_LIMIT + 10) {
        raw.push_str("unsafe\u{1b}[31m");
        raw.push_str(&home);
        raw.push_str(&"x".repeat(JOURNAL_LINE_CHAR_LIMIT + 100));
        raw.push('\n');
    }
    let lines = sanitize_journal(raw.as_bytes());

    assert_eq!(lines.len(), JOURNAL_LINE_LIMIT);
    assert!(lines.iter().all(|line| !line.contains('\u{1b}')));
    assert!(lines.iter().all(|line| !line.contains(&home)));
    assert!(lines
        .iter()
        .all(|line| line.chars().count() <= JOURNAL_LINE_CHAR_LIMIT));
}

#[test]
fn non_systemd_backends_report_informational_unavailable_logs() {
    for (label, source) in [
        ("dinit", DoctorLogSource::Dinit),
        ("runit", DoctorLogSource::Runit),
        ("s6-rc", DoctorLogSource::S6Rc),
    ] {
        let (result, check) = unavailable_manager_logs(label);
        assert_eq!(check.severity, DoctorSeverity::Note);
        assert!(matches!(
            result,
            DoctorLogResult::Unavailable { source: actual, .. } if actual == source
        ));
        assert!(check
            .details
            .as_deref()
            .is_some_and(|text| text.contains(label)));
    }
}

#[test]
fn unavailable_logs_never_become_doctor_errors() {
    let (_, check) = unavailable_logs(DoctorLogSource::Manual, "not configured");
    assert_ne!(check.severity, DoctorSeverity::Error);
}

#[tokio::test]
async fn systemd_without_verbose_mode_does_not_attempt_journal_collection() {
    let (result, check) = collect_systemd_logs(false).await;

    assert_eq!(check.severity, DoctorSeverity::Note);
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
async fn non_systemd_log_collection_executes_no_logger_command() {
    let root =
        std::env::temp_dir().join(format!("unixnotis-doctor-no-logger-{}", std::process::id()));
    let marker = root.join("called");
    std::fs::create_dir_all(&root).expect("create fake tool directory");
    for tool in ["journalctl", "dinitctl", "sv", "s6-log"] {
        let path = root.join(tool);
        std::fs::write(&path, format!("#!/bin/sh\ntouch '{}'\n", marker.display()))
            .expect("write fake logger command");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make fake logger executable");
    }
    let _tools = crate::system_tools::use_fake_tool_bin(&root);

    for manager in [
        ServiceManagerKind::Dinit,
        ServiceManagerKind::Runit,
        ServiceManagerKind::S6,
    ] {
        let (_result, check) = collect_logs(SelectedServiceManager::Managed(manager), true).await;
        assert_eq!(check.severity, DoctorSeverity::Note);
    }

    assert!(!marker.exists());
    std::fs::remove_dir_all(root).expect("remove fake tool directory");
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
    let _tools = crate::system_tools::use_fake_tool_bin(&root);

    let lines = read_recent_journal("unixnotis-daemon.service")
        .await
        .expect("oversized journal output should be truncated");

    assert_eq!(lines.len(), JOURNAL_LINE_LIMIT);
    assert!(lines.iter().all(|line| line.starts_with("bounded journal")));
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
    let _tools = crate::system_tools::use_fake_tool_bin(&root);

    let error = read_recent_journal("unixnotis-daemon.service")
        .await
        .expect_err("non-zero journal command must remain unavailable");

    assert!(error.contains("journalctl exited with status"));
    std::fs::remove_dir_all(root).expect("remove fake tool directory");
}
