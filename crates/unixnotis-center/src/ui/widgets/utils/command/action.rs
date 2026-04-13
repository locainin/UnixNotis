//! Action command helpers

use std::io;
use std::process::Output;

use gtk::glib;
use tracing::warn;
use unixnotis_core::{util, PanelDebugLevel};

use crate::debug;

use super::command_queue::enqueue_command;
use super::{resolve_command_plan, CommandKind};

pub(in crate::ui::widgets) fn run_command_capture_action_async(
    cmd: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    // Action capture keeps action priority while still reporting completion
    let (tx, rx) = async_channel::bounded(1);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        // Keep the receiver behavior consistent with the non-empty path
        let _ = tx.send_blocking(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command was empty",
        )));
        return rx;
    }
    let plan = resolve_command_plan(cmd, CommandKind::Action);
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue action-capture command: {snippet}")
    });
    enqueue_command(cmd.to_string(), plan, Some(tx));
    rx
}

pub(in crate::ui::widgets) fn run_action_command_with_completion<F>(
    cmd: String,
    context: &'static str,
    on_complete: F,
) where
    F: FnOnce(bool) + 'static,
{
    // One helper keeps action completion and failure handling the same across widgets
    let rx = run_command_capture_action_async(&cmd);
    let cmd_snip = util::log_snippet(&cmd);
    glib::MainContext::default().spawn_local(async move {
        let failed = match rx.recv().await {
            Ok(Ok(output)) => !output.status.success(),
            Ok(Err(err)) => {
                warn!(?err, command = %cmd, context, "action command failed");
                true
            }
            Err(_) => {
                warn!(command = %cmd, context, "action command response channel closed");
                true
            }
        };

        let level = if failed {
            PanelDebugLevel::Warn
        } else {
            PanelDebugLevel::Verbose
        };
        debug::log(level, || {
            format!("{context} completed failed={failed} cmd=\"{cmd_snip}\"")
        });

        on_complete(failed);
    });
}
