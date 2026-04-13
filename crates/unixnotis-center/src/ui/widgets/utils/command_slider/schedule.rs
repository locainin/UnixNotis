//! Slider set-command debounce and dispatch

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::super::run_command_capture_action_async;
use super::value::format_command_value;
use tracing::warn;
use unixnotis_core::PanelDebugLevel;

use crate::debug;

pub(super) fn schedule_command(
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    pending_value: Rc<Cell<Option<f64>>>,
    cmd_template: String,
    value: f64,
    step: f64,
    on_complete: Rc<dyn Fn(bool)>,
) {
    // Latest value wins while debounce timer is active
    pending_value.set(Some(value));
    if pending.borrow().is_some() {
        return;
    }

    let value_text = format_command_value(value, step);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("slider set scheduled value={value_text}")
    });
    let pending_guard = pending.clone();
    let pending_value = pending_value.clone();
    let id = glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
        // Drain pending state and execute the most recent queued command
        let value = pending_value.replace(None);
        let _ = pending_guard.borrow_mut().take();
        if let Some(value) = value {
            let formatted = cmd_template.replace("{value}", &format_command_value(value, step));
            let rx = run_command_capture_action_async(&formatted);
            let command = formatted.clone();
            let on_complete = on_complete.clone();
            glib::MainContext::default().spawn_local(async move {
                // Completion callback decides whether this action needs corrective refresh
                let failed = match rx.recv().await {
                    Ok(Ok(output)) => !output.status.success(),
                    Ok(Err(err)) => {
                        warn!(?err, command = %command, "slider set command failed");
                        true
                    }
                    Err(_) => true,
                };
                on_complete(failed);
            });
        }
        glib::ControlFlow::Break
    });
    *pending.borrow_mut() = Some(id);
}
