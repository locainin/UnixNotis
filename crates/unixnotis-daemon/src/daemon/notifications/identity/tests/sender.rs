use super::*;

#[cfg(target_os = "linux")]
#[test]
fn parse_process_start_time_handles_spaces_in_comm() {
    let stat = "42 (player with spaces) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654 20";
    assert_eq!(parse_process_start_time(stat), Some(987_654));
}

#[cfg(target_os = "linux")]
#[test]
fn parse_process_start_time_rejects_missing_or_invalid_fields() {
    assert!(parse_process_start_time("42 no-closing-paren").is_none());
    assert!(parse_process_start_time("42 (app) S 1 2 3").is_none());

    let stat = "42 (app) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 nope 20";
    assert!(parse_process_start_time(stat).is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn process_cmdline_parser_preserves_argument_boundaries_and_rejects_truncation() {
    assert_eq!(
        parse_process_cmdline(b"/usr/bin/python3\0/usr/share/app.py\0".to_vec()),
        Some(vec![
            b"/usr/bin/python3".to_vec(),
            b"/usr/share/app.py".to_vec(),
        ])
    );
    assert!(parse_process_cmdline(b"/usr/bin/python3\0truncated".to_vec()).is_none());
    assert!(parse_process_cmdline(Vec::new()).is_none());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn process_metadata_helpers_read_current_process_on_linux() {
    let pid = std::process::id();

    let exe = executable_evidence_for_pid(pid)
        .map(|evidence| evidence.canonical_path)
        .expect("current process executable should be readable");
    assert!(exe.is_absolute());

    let start_time = read_process_start_time(pid).expect("current process start time should exist");
    assert!(start_time > 1);
    let cmdline = read_process_cmdline(pid).expect("current process cmdline should exist");
    assert!(!cmdline.is_empty());
}

#[test]
fn stable_process_evidence_keeps_matching_lifetime_observations() {
    assert_eq!(
        stable_process_evidence(Some(42), Some("evidence"), Some(42)),
        (Some(42), Some("evidence"))
    );
}

#[test]
fn stable_process_evidence_discards_pid_reuse_or_missing_observations() {
    assert_eq!(
        stable_process_evidence(Some(42), Some("evidence"), Some(43)),
        (None, None)
    );
    assert_eq!(
        stable_process_evidence(None, Some("evidence"), None),
        (None, None)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn security_refresh_reloads_current_process_evidence() {
    let pid = std::process::id();
    let start_time = read_process_start_time(pid).expect("current process start time");
    let original = SenderMetadata {
        sender_pid: Some(pid),
        sender_start_time: Some(start_time),
        ..SenderMetadata::default()
    };

    let refreshed = refresh_sender_security_evidence(&original);

    assert_eq!(refreshed.sender_start_time, Some(start_time));
    assert!(refreshed.sender_executable_identity.is_some());
    assert!(refreshed.sender_cmdline.is_some());
}

#[cfg(target_os = "linux")]
#[test]
fn security_refresh_clears_evidence_for_a_stale_process_lifetime() {
    let pid = std::process::id();
    let stale_start = read_process_start_time(pid)
        .expect("current process start time")
        .saturating_add(1);
    let original = SenderMetadata {
        sender_pid: Some(pid),
        sender_start_time: Some(stale_start),
        sender_executable: Some("/usr/bin/trusted-app".to_string()),
        sender_executable_identity: Some(FileIdentity {
            device: 1,
            inode: 2,
            uid: 0,
            mode: 0o100_755,
        }),
        sender_cmdline: Some(vec![b"/usr/bin/trusted-app".to_vec()]),
        ..SenderMetadata::default()
    };

    let refreshed = refresh_sender_security_evidence(&original);

    assert!(refreshed.sender_start_time.is_none());
    assert!(refreshed.sender_executable.is_none());
    assert!(refreshed.sender_executable_identity.is_none());
    assert!(refreshed.sender_cmdline.is_none());
}
