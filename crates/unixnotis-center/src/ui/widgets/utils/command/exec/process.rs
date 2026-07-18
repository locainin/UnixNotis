//! Process-group termination helpers

#[cfg(unix)]
use rustix::process::{Pid, Signal};

pub(super) async fn kill_child_process(child: &mut tokio::process::Child) {
    // Signal the whole subtree before asking Tokio to reap the direct child
    if let Some(pid) = child.id() {
        kill_process_group(pid as i32);
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

pub(in crate::ui::widgets) fn kill_process_group(pid: i32) {
    if pid <= 0 {
        return;
    }
    #[cfg(unix)]
    {
        if let Some(pid) = Pid::from_raw(pid) {
            let _ = rustix::process::kill_process_group(pid, Signal::KILL);
        }
    }
}
