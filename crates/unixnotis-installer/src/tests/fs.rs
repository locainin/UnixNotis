//! Filesystem helpers shared by installer tests

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const FAKE_TOOL_DISPATCHER: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/fixtures/fake-tool");

pub(crate) fn write_executable(path: &Path, contents: &str) {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("tool");
    let script_path = path.with_file_name(format!(".{file_name}.script"));
    let temporary_script = path.with_file_name(format!(
        ".{file_name}.script.{}.{sequence}.tmp",
        std::process::id()
    ));
    let temporary_link = path.with_file_name(format!(
        ".{file_name}.link.{}.{sequence}.tmp",
        std::process::id()
    ));

    // Generated command logic remains non-executable data read by the stable dispatcher
    // This avoids ETXTBSY races on overlay filesystems used by CI containers
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_script)
        .expect("create temporary fake tool script");
    file.write_all(contents.as_bytes())
        .expect("write temporary fake tool script");
    file.sync_all().expect("sync temporary fake tool script");
    drop(file);

    // Both data and command links are replaced atomically so parallel readers see complete state
    fs::rename(&temporary_script, &script_path).expect("publish fake tool script atomically");
    symlink(FAKE_TOOL_DISPATCHER, &temporary_link).expect("create fake tool dispatcher link");
    fs::rename(&temporary_link, path).expect("publish fake tool dispatcher atomically");
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::write_executable;

    #[test]
    fn write_executable_can_replace_a_running_fixture_without_text_busy_errors() {
        let root = std::env::temp_dir().join(format!(
            "unixnotis-atomic-executable-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, super::Ordering::Relaxed)
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
}
