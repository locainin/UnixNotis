//! Stock theme migration regression tests

mod migration;
mod staging;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    std::env::current_dir()
        .expect("current directory should resolve")
        .join("target")
        .join(format!(
            "unixnotis-theme-stock-{name}-{}-{unique}",
            std::process::id()
        ))
}
