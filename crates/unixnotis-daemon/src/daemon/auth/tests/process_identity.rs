use super::process_identity::read_process_executable_path;
#[cfg(target_os = "linux")]
use super::process_identity::{
    parse_pidfd_process_id, pidfd_is_live, pidfd_matches_live_process, read_pidfd_info_bytes,
    read_pidfd_process_id, read_process_executable_path_from_pidfd,
};

#[cfg(target_os = "linux")]
#[tokio::test]
async fn read_process_executable_path_reads_current_process() {
    let exe = read_process_executable_path(std::process::id())
        .await
        .expect("current process executable should be readable");

    assert!(exe.is_absolute());
}

#[cfg(target_os = "linux")]
#[test]
fn parse_pidfd_process_id_requires_one_positive_pid_field() {
    assert_eq!(parse_pidfd_process_id("pos:\t0\nPid:\t4321\n"), Some(4321));
    assert_eq!(parse_pidfd_process_id("Pid:\t0\n"), None);
    assert_eq!(parse_pidfd_process_id("Pid:\t-1\n"), None);
    assert_eq!(parse_pidfd_process_id("NSpid:\t4321\n"), None);
    assert_eq!(parse_pidfd_process_id("Pid:\t1\nPid:\t2\n"), None);
}

#[cfg(target_os = "linux")]
#[test]
fn pidfd_info_reader_accepts_the_exact_limit_and_rejects_one_extra_byte() {
    use std::io::Cursor;

    assert_eq!(
        read_pidfd_info_bytes(Cursor::new(b"1234"), 4),
        Some(b"1234".to_vec())
    );
    assert_eq!(read_pidfd_info_bytes(Cursor::new(b"12345"), 4), None);
}

#[cfg(target_os = "linux")]
#[test]
fn pidfd_helpers_read_the_current_process_identity() {
    use rustix::process::{pidfd_open, Pid, PidfdFlags};

    let raw_pid = i32::try_from(std::process::id()).expect("process id should fit i32");
    let pid = Pid::from_raw(raw_pid).expect("current process id should be positive");
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).expect("current pidfd should open");

    assert!(pidfd_is_live(&pidfd));
    assert!(pidfd_matches_live_process(&pidfd, std::process::id()));
    assert!(!pidfd_matches_live_process(&pidfd, std::process::id() + 1));
    assert_eq!(read_pidfd_process_id(&pidfd), Some(std::process::id()));
    assert_eq!(
        read_process_executable_path_from_pidfd(&pidfd, std::process::id()),
        std::env::current_exe().ok()
    );
    assert_eq!(
        read_process_executable_path_from_pidfd(&pidfd, std::process::id() + 1),
        None
    );
}

#[cfg(target_os = "linux")]
#[test]
fn pidfd_executable_lookup_rejects_an_exited_process() {
    use std::process::Command;

    use rustix::process::{pidfd_open, Pid, PidfdFlags};

    let mut child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep child should start");
    let child_id = child.id();
    let raw_pid = i32::try_from(child_id).expect("child process id should fit i32");
    let pid = Pid::from_raw(raw_pid).expect("child process id should be positive");
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).expect("child pidfd should open");

    child.kill().expect("sleep child should stop");
    child.wait().expect("sleep child should be reaped");

    assert!(!pidfd_is_live(&pidfd));
    assert!(!pidfd_matches_live_process(&pidfd, child_id));
    assert_eq!(
        read_process_executable_path_from_pidfd(&pidfd, child_id),
        None
    );
}
