use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn temp_dir(label: &str) -> PathBuf {
    // Unique paths keep parallel test runs from sharing trial shim state
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "unixnotis-installer-trial-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}
