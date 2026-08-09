use std::process::Command;
use std::time::{Duration, Instant};

use super::super::output_bounded;

#[test]
fn bounded_probe_returns_complete_small_output() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf stdout; printf stderr >&2"]);

    let output = output_bounded(&mut command, Duration::from_secs(1), 64)
        .expect("bounded probe should finish");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"stdout");
    assert_eq!(output.stderr, b"stderr");
    assert!(!output.stdout_truncated);
    assert!(!output.stderr_truncated);
}

#[test]
fn bounded_probe_drains_but_does_not_retain_oversized_streams() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "head -c 65536 /dev/zero; head -c 65536 /dev/zero >&2"]);

    let output = output_bounded(&mut command, Duration::from_secs(1), 1_024)
        .expect("large bounded probe should finish without a pipe deadlock");

    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 1_024);
    assert_eq!(output.stderr.len(), 1_024);
    assert!(output.stdout_truncated);
    assert!(output.stderr_truncated);
}

#[test]
fn bounded_probe_kills_a_hung_process_group_at_the_deadline() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 30"]);
    let started = Instant::now();

    let error = output_bounded(&mut command, Duration::from_millis(25), 64)
        .expect_err("hung probe must time out");

    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "probe timeout must kill and reap the process group promptly"
    );
}

#[test]
fn bounded_probe_reaps_helpers_that_outlive_a_successful_parent() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 30 & exit 0"]);
    let started = Instant::now();

    let output = output_bounded(&mut command, Duration::from_secs(1), 64)
        .expect("completed probe should clean up its inherited pipe owners");

    assert!(output.status.success());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "background helpers must not extend the probe deadline"
    );
}
