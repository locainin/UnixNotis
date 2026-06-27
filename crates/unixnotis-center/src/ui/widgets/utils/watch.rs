//! Watcher lifecycle helpers for long-running widget commands.

use std::io::{self, BufRead};
use std::process::Child;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_channel::{TryRecvError, TrySendError};
use gtk::glib;
use tracing::warn;
use unixnotis_core::{util, PanelDebugLevel};

use crate::debug;
use crate::ui::perf_probe;

use super::command::{resolve_command_plan, CommandKind};
use super::watch_reaper::enqueue_watch_cleanup;

pub(in crate::ui::widgets) struct CommandWatch {
    // Command string retained for diagnostics during shutdown
    cmd: String,
    // Child process handle for lifecycle control
    child: Option<Child>,
    // Reader thread that consumes watch stdout
    thread: Option<std::thread::JoinHandle<()>>,
    // GTK-side debounce task for event coalescing
    task: Option<glib::JoinHandle<()>>,
    // Tracks whether the watch process is still emitting events.
    active: Arc<AtomicBool>,
}

impl Drop for CommandWatch {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        if let Some(task) = self.task.take() {
            task.abort();
        }
        let cmd = std::mem::take(&mut self.cmd);
        let child = self.child.take();
        let thread = self.thread.take();

        if child.is_none() && thread.is_none() {
            return;
        }

        // Queue teardown onto the shared reaper so drop stays quick under churn
        enqueue_watch_cleanup(cmd, child, thread);
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
    // Spawn watch command with stdout piped so events can be consumed
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

    // Single-slot channel coalesces bursts from noisy watch commands.
    let (tx, rx) = async_channel::bounded::<()>(1);
    let on_event = Rc::new(on_event);
    let debounce = Duration::from_millis(120);
    let active = Arc::new(AtomicBool::new(true));
    let task = glib::MainContext::default().spawn_local({
        let on_event = on_event.clone();
        let cmd = cmd_string.clone();
        async move {
            // Debounce loop coalesces bursts into fewer refresh callbacks
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
                perf_probe::watch_event();
                on_event();
            }
        }
    });

    let thread = std::thread::spawn({
        let cmd = cmd_for_thread;
        let active = Arc::clone(&active);
        move || {
            let reader = io::BufReader::new(stdout);
            let mut events = 0usize;
            for line in reader.lines() {
                let line = match line {
                    Ok(line) => line,
                    Err(_) => break,
                };
                if !should_emit_watch_event(&cmd, &line) {
                    continue;
                }
                events += 1;
                match tx.try_send(()) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {}
                    Err(TrySendError::Closed(_)) => break,
                }
            }
            active.store(false, Ordering::Release);
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
        active,
    })
}

impl CommandWatch {
    pub(in crate::ui::widgets) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

fn should_emit_watch_event(cmd: &str, line: &str) -> bool {
    let cmd = cmd.trim();
    let line = line.trim();
    if line.is_empty() {
        return false;
    }

    // pactl subscribe emits events for all server activity; filter to sink/server changes.
    if cmd.starts_with("pactl subscribe") {
        let line = line.to_ascii_lowercase();
        return contains_token(&line, " on sink") || contains_token(&line, " on server");
    }
    if cmd.starts_with("nmcli") && cmd.contains(" monitor") {
        // nmcli prints a startup status line before real NetworkManager events.
        return !matches!(
            line,
            "NetworkManager is running" | "NetworkManager is not running"
        );
    }
    if cmd.starts_with("udevadm monitor") {
        // udevadm prints a banner describing the monitored source before events arrive.
        return !line.starts_with("monitor will print the received events for:");
    }
    if cmd.starts_with("dbus-monitor") {
        // dbus-monitor announces its own unique-name connection before watched signals
        // Keep user-requested D-Bus name lifecycle events unless the line has that startup shape
        return !is_dbus_monitor_startup_lifecycle(line);
    }
    true
}

fn is_dbus_monitor_startup_lifecycle(line: &str) -> bool {
    let has_lifecycle_member =
        line.contains("member=NameAcquired") || line.contains("member=NameLost");

    // Startup chatter is about the monitor process receiving a unique bus name
    // Other org.freedesktop.DBus lifecycle signals may be meaningful watch events
    has_lifecycle_member
        && line.contains("sender=org.freedesktop.DBus")
        && line.contains("-> destination=:")
        && line.contains("path=/org/freedesktop/DBus")
        && line.contains("interface=org.freedesktop.DBus")
        && line.contains("string \":")
}

fn contains_token(line: &str, token: &str) -> bool {
    // Ensure the token is followed by whitespace or end-of-line to avoid matching "sink-input".
    let Some(index) = line.find(token) else {
        return false;
    };
    let tail = &line[index + token.len()..];
    tail.is_empty() || tail.starts_with(char::is_whitespace)
}

#[cfg(test)]
#[path = "tests/watch.rs"]
mod tests;
