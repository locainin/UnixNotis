//! Toggle async state refresh helpers
//!
//! This module isolates command execution, parsing, and bounded retry behavior

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use tracing::warn;
use unixnotis_core::{css::hooks, util, PanelDebugLevel};

use super::super::utils::run_command_capture_status_async;
use crate::debug;
use crate::ui::perf_probe;

// Staggered retry delays keep UI responsive without long-lived polling loops
const TOGGLE_REFRESH_DELAYS_MS: &[u64] = &[0, 50, 100, 200, 400, 800];

#[derive(Clone)]
pub(super) struct ToggleRefreshGate {
    // True while one state probe is already running
    in_flight: Rc<Cell<bool>>,
    // One trailing refresh is enough to cover watch bursts
    pending: Rc<Cell<bool>>,
}

impl ToggleRefreshGate {
    pub(super) fn new() -> Self {
        Self {
            in_flight: Rc::new(Cell::new(false)),
            pending: Rc::new(Cell::new(false)),
        }
    }

    fn begin_or_queue(&self) -> bool {
        if self.in_flight.get() {
            self.pending.set(true);
            return false;
        }
        self.in_flight.set(true);
        true
    }

    fn finish(&self) -> bool {
        self.in_flight.set(false);
        self.pending.replace(false)
    }
}

pub(super) fn refresh_toggle_state(
    cmd: &str,
    button: &gtk::ToggleButton,
    guard: &Rc<Cell<bool>>,
    refresh_gen: &Rc<Cell<u64>>,
    refresh_gate: &ToggleRefreshGate,
) {
    // Bursty watch events only need one running probe and one trailing probe
    if !refresh_gate.begin_or_queue() {
        perf_probe::toggle_refresh_queued();
        let cmd_snip = util::log_snippet(cmd);
        debug::log(PanelDebugLevel::Verbose, || {
            format!("toggle refresh queued while in flight cmd=\"{cmd_snip}\"")
        });
        return;
    }
    perf_probe::toggle_refresh_start();

    // Periodic refresh path keeps UI aligned with external command state
    let cmd = cmd.to_string();

    // Each refresh claims a generation so stale tasks cannot overwrite newer state
    let gen = next_refresh_generation(refresh_gen);
    let button = button.clone();
    let guard = guard.clone();
    let refresh_gen = refresh_gen.clone();
    let refresh_gate = refresh_gate.clone();
    let refresh_cmd = cmd.clone();
    let cmd_snip = util::log_snippet(&cmd);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("toggle refresh start cmd=\"{cmd_snip}\"")
    });

    glib::MainContext::default().spawn_local(async move {
        // Single probe path is used for periodic refresh and watch-trigger refresh
        let Some(active) = fetch_toggle_state(&cmd, true).await else {
            finish_toggle_refresh(refresh_cmd, button, guard, refresh_gen, refresh_gate);
            return;
        };

        // Drop stale result when a newer refresh has already started
        if refresh_gen.get() != gen {
            finish_toggle_refresh(refresh_cmd, button, guard, refresh_gen, refresh_gate);
            return;
        }

        if button.is_active() != active {
            // Guard blocks feedback loops through connect_toggled
            guard.set(true);
            perf_probe::toggle_state_write();
            button.set_active(active);
            guard.set(false);
        }
        apply_active_class(&button, active);

        finish_toggle_refresh(refresh_cmd, button, guard, refresh_gen, refresh_gate);
    });
}

pub(super) fn schedule_toggle_refresh_with_retry(
    state_cmd: String,
    expected: bool,
    button: gtk::ToggleButton,
    guard: Rc<Cell<bool>>,
    refresh_gen: Rc<Cell<u64>>,
) {
    // Post-action retry path closes race windows where backend state lags UI input
    // Bounded retries reconcile optimistic UI state with eventually-consistent commands
    let gen = next_refresh_generation(&refresh_gen);

    // Weak refs avoid extending widget lifetimes from detached async tasks
    let button_weak = button.downgrade();
    let guard_weak = Rc::downgrade(&guard);
    let refresh_gen_weak = Rc::downgrade(&refresh_gen);

    glib::MainContext::default().spawn_local(async move {
        // Retry cadence is short and bounded to avoid long-running background churn
        for (attempt, delay_ms) in TOGGLE_REFRESH_DELAYS_MS.iter().enumerate() {
            // Delay sequence smooths transient backend lag
            if *delay_ms > 0 {
                glib::timeout_future(Duration::from_millis(*delay_ms)).await;
            }

            let Some(refresh_gen) = refresh_gen_weak.upgrade() else {
                // Parent state dropped, stop work immediately
                return;
            };
            if refresh_gen.get() != gen {
                return;
            }

            // Keep warnings bounded to the first failed probe per action
            let log_failures = attempt == 0;
            let Some(active) = fetch_toggle_state(&state_cmd, log_failures).await else {
                // Probe failed, continue to next retry window
                continue;
            };

            if refresh_gen.get() != gen {
                return;
            }

            // Widgets may have been destroyed while command was running
            let (Some(button), Some(guard)) = (button_weak.upgrade(), guard_weak.upgrade()) else {
                return;
            };

            if button.is_active() != active {
                // Apply corrected state without retriggering command dispatch
                guard.set(true);
                perf_probe::toggle_state_write();
                button.set_active(active);
                guard.set(false);
            }
            apply_active_class(&button, active);

            if active == expected {
                // Stop retrying once backend and UI agree
                return;
            }
        }
    });
}

fn apply_active_class(button: &gtk::ToggleButton, active: bool) {
    if active {
        if !button.has_css_class(hooks::shared_state::ACTIVE) {
            perf_probe::toggle_class_write();
            button.add_css_class(hooks::shared_state::ACTIVE);
        }
    } else if button.has_css_class(hooks::shared_state::ACTIVE) {
        perf_probe::toggle_class_write();
        button.remove_css_class(hooks::shared_state::ACTIVE);
    }
}

async fn fetch_toggle_state(cmd: &str, log_failures: bool) -> Option<bool> {
    // Shared fetch routine is used by both periodic refresh and retry path
    // Command helper returns receiver so execution stays off the GTK thread
    let rx = run_command_capture_status_async(cmd);

    let output = match rx.recv().await {
        Ok(output) => output,
        // Channel close means command worker is unavailable
        Err(_) => return None,
    };

    let output = match output {
        Ok(output) => output,
        Err(err) => {
            if log_failures {
                warn!(?cmd, ?err, "toggle state command failed");
            }
            // Parse path is skipped when command invocation itself fails
            return None;
        }
    };

    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Empty stdout falls back to exit status semantics
    let active = if stdout.trim().is_empty() {
        success
    } else {
        parse_toggle_state(&stdout)
    };

    Some(active)
}

fn finish_toggle_refresh(
    cmd: String,
    button: gtk::ToggleButton,
    guard: Rc<Cell<bool>>,
    refresh_gen: Rc<Cell<u64>>,
    refresh_gate: ToggleRefreshGate,
) {
    // One queued refresh is enough to bring the toggle back to the newest state
    if refresh_gate.finish() {
        let cmd_snip = util::log_snippet(&cmd);
        debug::log(PanelDebugLevel::Verbose, || {
            format!("toggle refresh consumed pending request cmd=\"{cmd_snip}\"")
        });
        refresh_toggle_state(&cmd, &button, &guard, &refresh_gen, &refresh_gate);
    }
}

fn next_refresh_generation(refresh_gen: &Rc<Cell<u64>>) -> u64 {
    // Wrap naturally so very long sessions keep the same stale-result semantics
    let next = refresh_gen.get().wrapping_add(1);
    refresh_gen.set(next);
    next
}

fn parse_toggle_state(output: &str) -> bool {
    // Handle bluetoothctl style structured output first for predictable power semantics
    for line in output.lines() {
        let lower = line.trim().to_ascii_lowercase();
        if lower.contains("powered") || lower.contains("powerstate") {
            if lower.contains("no")
                || lower.contains("off")
                || lower.contains("false")
                || lower.contains("disabled")
            {
                return false;
            }
            if lower.contains("yes")
                || lower.contains("on")
                || lower.contains("true")
                || lower.contains("enabled")
            {
                return true;
            }
        }
    }

    let value = output.trim().to_ascii_lowercase();

    // systemctl is-active style output has strong explicit states
    if matches!(value.as_str(), "active" | "activated") {
        return true;
    }
    if matches!(value.as_str(), "inactive" | "failed" | "dead") {
        return false;
    }

    // Single-token affirmative results are treated as enabled
    if matches!(
        value.as_str(),
        "1" | "on" | "yes" | "true" | "enabled" | "up"
    ) {
        return true;
    }

    // Tokenized fallback catches mixed plain-text command outputs
    value
        // Non-alphanumeric separators cover outputs like "state: on"
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        // Any affirmative token is treated as enabled to handle mixed output formats
        .any(|token| matches!(token, "on" | "yes" | "true" | "enabled" | "up" | "active"))
}

#[cfg(test)]
mod tests {
    use super::ToggleRefreshGate;

    #[test]
    fn refresh_gate_queues_one_trailing_refresh() {
        let gate = ToggleRefreshGate::new();

        assert!(gate.begin_or_queue());
        assert!(!gate.begin_or_queue());
        assert!(gate.finish());
    }

    #[test]
    fn refresh_gate_clears_pending_after_finish() {
        let gate = ToggleRefreshGate::new();

        assert!(gate.begin_or_queue());
        assert!(!gate.begin_or_queue());
        assert!(gate.finish());
        assert!(!gate.finish());
        assert!(gate.begin_or_queue());
    }

    #[test]
    fn refresh_gate_does_not_stack_multiple_pending_runs() {
        let gate = ToggleRefreshGate::new();

        assert!(gate.begin_or_queue());
        assert!(!gate.begin_or_queue());
        assert!(!gate.begin_or_queue());
        assert!(gate.finish());
        assert!(!gate.finish());
    }
}
