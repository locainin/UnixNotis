//! Watcher lifecycle helpers for long-running widget commands.

use std::io::{self, BufRead};
use std::process::Child;
use std::rc::Rc;
use std::time::Duration;

use async_channel::TryRecvError;
use gtk::glib;
use tracing::warn;
use unixnotis_core::util;
use unixnotis_core::PanelDebugLevel;

use crate::debug;

use super::command_utils::{kill_process_group, resolve_command_plan, CommandKind};

pub(in crate::ui::widgets) struct CommandWatch {
    cmd: String,
    child: Option<Child>,
    thread: Option<std::thread::JoinHandle<()>>,
    task: Option<glib::JoinHandle<()>>,
}

impl Drop for CommandWatch {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        let cmd = std::mem::take(&mut self.cmd);
        let child = self.child.take();
        let thread = self.thread.take();

        if child.is_none() && thread.is_none() {
            return;
        }

        // Cleanup runs off the GTK thread to avoid UI stalls on process shutdown.
        std::thread::spawn(move || {
            if let Some(mut child) = child {
                let pid = child.id() as i32;
                kill_process_group(pid);
                let _ = child.kill();
                let _ = child.wait();
            }
            if let Some(handle) = thread {
                let _ = handle.join();
            }
            debug::log(PanelDebugLevel::Info, || {
                let snippet = util::log_snippet(&cmd);
                format!("watch cleanup complete: {snippet}")
            });
        });
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
    debug::log(PanelDebugLevel::Info, || {
        let snippet = util::log_snippet(cmd);
        format!("watch start: {snippet}")
    });

    let plan = resolve_command_plan(cmd, CommandKind::Slow);
    let cmd_string = cmd.to_string();
    let cmd_for_thread = cmd_string.clone();
    let mut child = match plan.spawn_watch_command(cmd) {
        Ok(child) => child,
        Err(err) => {
            let snippet = util::log_snippet(cmd);
            warn!(command = %snippet, ?err, "watch command failed to start");
            return None;
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let snippet = util::log_snippet(cmd);
            warn!(command = %snippet, "watch command missing stdout");
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
        let cmd = cmd_string.clone();
        async move {
            while rx.recv().await.is_ok() {
                loop {
                    glib::timeout_future(debounce).await;
                    match rx.try_recv() {
                        Ok(_) => while rx.try_recv().is_ok() {},
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Closed) => return,
                    }
                }
                debug::log(PanelDebugLevel::Verbose, || {
                    let snippet = util::log_snippet(&cmd);
                    format!("watch event: {snippet}")
                });
                on_event();
            }
        }
    });

    let thread = std::thread::spawn({
        let cmd = cmd_for_thread;
        move || {
            let reader = io::BufReader::new(stdout);
            let mut events = 0usize;
            for line in reader.lines() {
                if line.is_err() {
                    break;
                }
                events += 1;
                if tx.send_blocking(()).is_err() {
                    break;
                }
            }
            debug::log(PanelDebugLevel::Info, || {
                let snippet = util::log_snippet(&cmd);
                format!("watch stopped: {snippet} (events={events})")
            });
        }
    });

    Some(CommandWatch {
        cmd: cmd_string,
        child: Some(child),
        thread: Some(thread),
        task: Some(task),
    })
}
