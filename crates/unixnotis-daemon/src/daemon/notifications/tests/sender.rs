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
#[tokio::test]
async fn process_metadata_helpers_read_current_process_on_linux() {
    let pid = std::process::id();

    let exe = read_process_executable_path(pid)
        .await
        .expect("current process executable should be readable");
    assert!(exe.is_absolute());

    let start_time = read_process_start_time(pid).expect("current process start time should exist");
    assert!(start_time > 1);
}
