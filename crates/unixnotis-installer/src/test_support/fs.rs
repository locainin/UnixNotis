//! Filesystem helpers shared by installer tests

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const FAKE_TOOL_DISPATCHER: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/test_support/fixtures/fake-tool"
);

pub fn write_executable(path: &Path, contents: &str) {
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
