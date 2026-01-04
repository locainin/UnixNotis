//! Watcher lifecycle helpers for long-running widget commands.

use std::io::{self, BufRead};
use std::process::Child;
use std::rc::Rc;
use std::time::Duration;

use async_channel::TryRecvError;
use gtk::glib;
use tracing::warn;

use super::command_utils::{kill_process_group, resolve_command_plan, CommandKind};

pub(in crate::ui::widgets) struct CommandWatch {
    child: Option<Child>,
    thread: Option<std::thread::JoinHandle<()>>,
    task: Option<glib::JoinHandle<()>>,
}

impl Drop for CommandWatch {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        if let Some(mut child) = self.child.take() {
            let pid = child.id() as i32;
            kill_process_group(pid);
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

pub(in crate::ui::widgets) fn start_command_watch<F: Fn() + 'static>(
    cmd: &str,
    on_event: F,
) -> Option<CommandWatch> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        warn!("watch command was empty");
        return None;
    }

    let plan = resolve_command_plan(cmd, CommandKind::Slow);
    let mut child = match plan.spawn_watch_command(cmd) {
        Ok(child) => child,
        Err(err) => {
            warn!(command = ?cmd, ?err, "watch command failed to start");
            return None;
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            warn!(command = ?cmd, "watch command missing stdout");
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };

    let (tx, rx) = async_channel::unbounded::<()>();
    let on_event = Rc::new(on_event);
    let debounce = Duration::from_millis(120);
    let task = glib::MainContext::default().spawn_local({
        let on_event = on_event.clone();
        async move {
            while rx.recv().await.is_ok() {
                loop {
                    glib::timeout_future(debounce).await;
                    match rx.try_recv() {
                        Ok(_) => {
                            while rx.try_recv().is_ok() {}
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Closed) => return,
                    }
                }
                on_event();
            }
        }
    });

    let thread = std::thread::spawn(move || {
        let reader = io::BufReader::new(stdout);
        for line in reader.lines() {
            if line.is_err() {
                break;
            }
            if tx.send_blocking(()).is_err() {
                break;
            }
        }
    });

    Some(CommandWatch {
        child: Some(child),
        thread: Some(thread),
        task: Some(task),
    })
}
