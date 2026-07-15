use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::super::fs::{write_executable, TEMP_SEQUENCE};
use std::sync::atomic::Ordering;

#[test]
fn write_executable_can_replace_a_running_fixture_without_text_busy_errors() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-atomic-executable-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let tool = root.join("tool");
    let started = root.join("started");
    let release = root.join("release");
    write_executable(
        &tool,
        &format!(
            "#!/bin/sh\nprintf started > '{}'\nwhile [ ! -e '{}' ]; do :; done\nprintf old\n",
            started.display(),
            release.display()
        ),
    );
    let mut running = Command::new(&tool)
        .stdout(Stdio::null())
        .spawn()
        .expect("start old fixture");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !started.exists() {
        if Instant::now() >= deadline {
            let _ = running.kill();
            panic!("old fixture did not start");
        }
        std::thread::yield_now();
    }

    // Publishing a replacement must not replace an executable inode under the running child
    write_executable(&tool, "#!/bin/sh\nprintf new\n");
    let replacement = Command::new(&tool)
        .output()
        .expect("run replacement fixture");
    std::fs::write(&release, b"release\n").expect("release old fixture");
    let old_status = running.wait().expect("wait for old fixture");

    assert!(old_status.success());
    assert!(replacement.status.success());
    assert_eq!(String::from_utf8_lossy(&replacement.stdout), "new");
    let _ = std::fs::remove_dir_all(root);
}
