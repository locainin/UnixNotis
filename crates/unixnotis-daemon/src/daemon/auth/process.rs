//! Process metadata helpers for authorization checks

use std::path::PathBuf;

pub(in crate::daemon) async fn read_process_executable_path(pid: u32) -> Option<PathBuf> {
    // Linux exposes the real executable path via /proc
    let path = format!("/proc/{pid}/exe");
    tokio::fs::read_link(path).await.ok()
}
