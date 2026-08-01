use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::process::{
    run_package_query, run_package_query_with_timeout, PackageQueryFailure,
};

#[test]
fn package_query_deadline_stops_a_stalled_provider() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 2"]);
    let started = Instant::now();

    let output = run_package_query_with_timeout(&mut command, 1024, Duration::from_millis(20));

    assert!(
        matches!(output, Err(PackageQueryFailure::Timeout)),
        "a stalled provider should report its deadline"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the package provider deadline should stop a stalled process promptly"
    );
}

#[test]
fn package_query_rejects_output_beyond_the_declared_limit() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 12345"]);

    assert!(
        run_package_query_with_timeout(&mut command, 4, Duration::from_secs(1)).is_err(),
        "oversized provider output must fail closed"
    );
}

#[test]
fn package_query_accepts_successful_output_at_the_exact_limit() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 1234"]);

    let output = run_package_query(&mut command, 4)
        .expect("successful provider output at the exact limit should be retained");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"1234");
}

#[test]
fn package_query_returns_when_descendant_holds_stdout_open() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "(sleep 2) & exit 0"]);
    let started = Instant::now();

    let output = run_package_query_with_timeout(&mut command, 1024, Duration::from_millis(100));

    assert!(
        matches!(output, Err(PackageQueryFailure::PipeDrainTimeout)),
        "an inherited output pipe should report a bounded drain timeout"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "an inherited output pipe must not block desktop-index construction"
    );
}
#[test]
fn timed_out_package_provider_is_terminated_before_returning() {
    let serial = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should follow the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-package-timeout-{}-{}",
        std::process::id(),
        serial
    ));
    fs::create_dir_all(&root).expect("package timeout test root should be created");
    let pid_file = root.join("provider.pid");
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "printf '%s' \"$$\" > \"$1\"; exec sleep 2",
            "unixnotis-package-timeout",
        ])
        .arg(&pid_file);

    let result = run_package_query_with_timeout(&mut command, 1024, Duration::from_millis(100));
    assert!(matches!(result, Err(PackageQueryFailure::Timeout)));
    let pid = fs::read_to_string(&pid_file).expect("provider should publish its process id");
    let process_path = Path::new("/proc").join(pid.trim());
    let reap_deadline = Instant::now() + Duration::from_millis(250);
    while process_path.exists() && Instant::now() < reap_deadline {
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(
        !process_path.exists(),
        "a timed-out provider must not continue after the ownership query returns"
    );
    fs::remove_dir_all(root).expect("package timeout test root should be removable");
}
