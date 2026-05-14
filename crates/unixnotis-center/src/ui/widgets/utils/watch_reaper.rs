//! Shared cleanup worker for long-running watch commands

use std::process::Child;
use std::sync::{mpsc, OnceLock};
use std::thread::{self, JoinHandle};

use tracing::warn;
use unixnotis_core::{util, PanelDebugLevel};

use crate::debug;

use super::command::kill_process_group;

struct CleanupJob {
    // Command text is kept for readable cleanup logs
    cmd: String,
    // Child process may still need kill and wait
    child: Option<Child>,
    // Reader thread should be joined after stdout closes
    reader_thread: Option<JoinHandle<()>>,
}

pub(super) fn enqueue_watch_cleanup(
    cmd: String,
    child: Option<Child>,
    reader_thread: Option<JoinHandle<()>>,
) {
    let job = CleanupJob {
        cmd,
        child,
        reader_thread,
    };

    if let Some(sender) = reaper_sender() {
        match sender.send(job) {
            Ok(()) => return,
            Err(err) => {
                // A dead reaper should not leak child processes during teardown
                warn!("watch reaper channel closed; falling back to direct cleanup");
                spawn_fallback_cleanup(err.0);
                return;
            }
        }
    }

    spawn_fallback_cleanup(job);
}

fn reaper_sender() -> Option<&'static mpsc::Sender<CleanupJob>> {
    static REAPER: OnceLock<Option<mpsc::Sender<CleanupJob>>> = OnceLock::new();

    REAPER
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel::<CleanupJob>();
            let spawn_result = thread::Builder::new()
                .name("unixnotis-watch-reaper".to_string())
                .spawn(move || {
                    // One worker handles teardown in order and avoids thread bursts
                    while let Ok(job) = rx.recv() {
                        run_cleanup(job);
                    }
                });

            match spawn_result {
                Ok(_) => Some(tx),
                Err(err) => {
                    warn!(?err, "failed to start watch reaper");
                    None
                }
            }
        })
        .as_ref()
}

fn spawn_fallback_cleanup(job: CleanupJob) {
    // Last-resort cleanup keeps drop non-blocking even if the shared reaper is unavailable
    if let Err(err) = thread::Builder::new()
        .name("unixnotis-watch-cleanup".to_string())
        .spawn(move || run_cleanup(job))
    {
        warn!(?err, "failed to spawn fallback watch cleanup thread");
    }
}

fn run_cleanup(job: CleanupJob) {
    let CleanupJob {
        cmd,
        child,
        reader_thread,
    } = job;

    if let Some(mut child) = child {
        // Process-group kill keeps shell wrappers and child trees from surviving teardown
        let pid = child.id() as i32;
        kill_process_group(pid);
        let _ = child.kill();
        let _ = child.wait();
    }

    if let Some(handle) = reader_thread {
        // Join after the child exits so the stdout reader can finish cleanly
        let _ = handle.join();
    }

    debug::log(PanelDebugLevel::Info, || {
        let snippet = util::log_snippet(&cmd);
        format!("watch cleanup complete: {snippet}")
    });
}

#[cfg(test)]
#[path = "tests/watch_reaper.rs"]
mod tests;
