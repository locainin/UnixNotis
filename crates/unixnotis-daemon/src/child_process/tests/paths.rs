use super::*;
use crate::test_support::env_lock;

fn test_executable_dir() -> PathBuf {
    std::env::current_exe()
        .expect("current test executable")
        .parent()
        .expect("test executable should have a parent")
        .to_path_buf()
}

fn write_sibling(name: &str) -> PathBuf {
    let path = test_executable_dir().join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write sibling binary");
    path
}

#[cfg(target_os = "linux")]
fn process_is_running(pid: u32) -> bool {
    let stat_path = format!("/proc/{pid}/stat");
    let Ok(stat) = std::fs::read_to_string(stat_path) else {
        return false;
    };

    // The process name may contain spaces and parentheses, so parse after its final ')'
    let Some(state) = stat
        .rsplit_once(") ")
        .and_then(|(_, rest)| rest.chars().next())
    else {
        return false;
    };

    // A zombie has exited but can remain visible until the reaper collects it
    !matches!(state, 'Z' | 'X')
}

#[test]
fn resolve_sibling_binary_prefers_exact_sibling_name() {
    let _guard = env_lock();
    let path = write_sibling("unixnotis-popups");

    assert_eq!(
        resolve_sibling_binary("unixnotis-popups"),
        Some(path.clone())
    );
    assert_eq!(resolve_popups_path(), Some(path.clone()));

    let _ = std::fs::remove_file(path);
}

#[test]
fn resolve_sibling_binary_falls_back_to_exe_suffix() {
    let _guard = env_lock();
    let path = write_sibling("unixnotis-center.exe");
    let exact = test_executable_dir().join("unixnotis-center");
    let _ = std::fs::remove_file(&exact);

    assert_eq!(
        resolve_sibling_binary("unixnotis-center"),
        Some(path.clone())
    );
    assert_eq!(resolve_center_path(), Some(path.clone()));

    let _ = std::fs::remove_file(path);
}

#[test]
fn resolve_sibling_binary_returns_none_when_no_sibling_exists() {
    let _guard = env_lock();
    let exact = test_executable_dir().join("unixnotis-missing");
    let exe = test_executable_dir().join("unixnotis-missing.exe");
    let _ = std::fs::remove_file(&exact);
    let _ = std::fs::remove_file(&exe);

    assert!(resolve_sibling_binary("unixnotis-missing").is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn parent_death_signal_terminates_a_child_when_its_launcher_exits() {
    let _guard = env_lock();
    let marker_path = std::env::temp_dir().join(format!(
        "unixnotis-pdeath-{}-{}.pid",
        std::process::id(),
        std::time::Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_file(&marker_path);
    let helper = std::env::current_exe().expect("current test executable");
    let status = std::process::Command::new(helper)
        .args([
            "--exact",
            "child_process::paths::tests::parent_death_signal_child_helper",
            "--nocapture",
        ])
        .env("UNIXNOTIS_PDEATH_MARKER", &marker_path)
        .status()
        .expect("launch parent-death helper");
    assert!(status.success(), "helper test failed: {status}");

    let pid = std::fs::read_to_string(&marker_path)
        .expect("helper should publish the child pid")
        .trim()
        .parse::<u32>()
        .expect("child pid should be numeric");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while process_is_running(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if process_is_running(pid) {
        // Clean up a failed mutation so the test cannot leak a long-running shell
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    let cleanup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while process_is_running(pid) && std::time::Instant::now() < cleanup_deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!process_is_running(pid), "child survived launcher exit");
    let _ = std::fs::remove_file(marker_path);
}

#[cfg(target_os = "linux")]
#[test]
fn parent_death_signal_rejects_a_changed_parent_before_exec() {
    let _guard = env_lock();
    let helper = std::env::current_exe().expect("current test executable");
    let status = std::process::Command::new(helper)
        .args([
            "--exact",
            "child_process::paths::tests::parent_death_signal_child_helper",
            "--nocapture",
        ])
        .env("UNIXNOTIS_PDEATH_EXPECT_MISMATCH", "1")
        .status()
        .expect("launch parent-death race helper");
    assert!(status.success(), "mismatch helper failed: {status}");
}

#[cfg(target_os = "linux")]
#[test]
fn parent_death_signal_child_helper() {
    let Some(marker) = std::env::var_os("UNIXNOTIS_PDEATH_MARKER") else {
        return;
    };
    let mut command = tokio::process::Command::new("/bin/sh");
    command
        .args(["-c", "trap 'exit 0' TERM; while :; do sleep 1; done"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let expected_parent_pid = if std::env::var_os("UNIXNOTIS_PDEATH_EXPECT_MISMATCH").is_some() {
        std::process::id().saturating_add(1)
    } else {
        std::process::id()
    };
    apply_parent_death_signal(&mut command, expected_parent_pid);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .expect("build helper runtime");
    runtime.block_on(async move {
        let child = command.spawn();
        if std::env::var_os("UNIXNOTIS_PDEATH_EXPECT_MISMATCH").is_some() {
            assert!(child.is_err(), "mismatched parent must fail before exec");
            return;
        }
        let child = child.expect("spawn supervised child");
        std::fs::write(marker, child.id().expect("child pid").to_string())
            .expect("write child pid marker");
    });
}
