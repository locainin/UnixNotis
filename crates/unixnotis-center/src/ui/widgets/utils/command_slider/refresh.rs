//! Slider refresh state and async refresh execution

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use tracing::warn;
use unixnotis_core::{util, NumericParseMode, PanelDebugLevel};

use super::super::run_command_capture_status_async;
use super::super::slider_parse::{format_value, parse_muted, parse_numeric};
use super::value::slider_value_changed;
use crate::debug;

#[derive(Clone)]
pub(super) struct SliderRefreshState {
    // Slider updated from command output
    pub(super) scale: gtk::Scale,
    // Label kept in sync with the slider
    pub(super) label: gtk::Label,
    // Icon image updated after refresh
    pub(super) icon_image: gtk::Image,
    // Guard stops refresh writes from triggering another set command
    pub(super) updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    pub(super) refresh_gen: Rc<Cell<u64>>,
    // Normal icon shown when not muted
    pub(super) icon_name: String,
    // Optional icon used when muted
    pub(super) icon_muted: Option<String>,
    // Local gate keeps refresh bursts bounded to one running and one pending
    pub(super) gate: SliderRefreshGate,
}

#[derive(Clone)]
pub(super) struct SliderRefreshMeta {
    // Non-widget refresh state that is safe to hold across signal closures
    pub(super) updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    pub(super) refresh_gen: Rc<Cell<u64>>,
    // Normal icon shown when not muted
    pub(super) icon_name: String,
    // Optional icon used when muted
    pub(super) icon_muted: Option<String>,
    // Local gate keeps refresh bursts bounded to one running and one pending
    pub(super) gate: SliderRefreshGate,
}

#[derive(Clone)]
pub(super) struct SliderRefreshGate {
    // True while one refresh command is already running
    in_flight: Rc<Cell<bool>>,
    // Remembers one trailing refresh request during bursts
    pending: Rc<Cell<bool>>,
}

impl SliderRefreshGate {
    pub(super) fn new() -> Self {
        Self {
            in_flight: Rc::new(Cell::new(false)),
            pending: Rc::new(Cell::new(false)),
        }
    }

    pub(super) fn begin_or_queue(&self) -> bool {
        if self.in_flight.get() {
            // One trailing refresh is enough to cover a burst of incoming requests
            self.pending.set(true);
            return false;
        }
        self.in_flight.set(true);
        true
    }

    pub(super) fn finish(&self) -> bool {
        self.in_flight.set(false);
        self.pending.replace(false)
    }
}

pub(super) fn request_refresh(
    cmd: String,
    min: f64,
    max: f64,
    step: f64,
    parse_mode: NumericParseMode,
    refresh: SliderRefreshState,
) {
    // Collapse bursty requests into one running refresh and one trailing refresh
    if !refresh.gate.begin_or_queue() {
        let cmd_snip = util::log_snippet(&cmd);
        debug::log(PanelDebugLevel::Verbose, || {
            format!("slider refresh queued while in flight cmd=\"{}\"", cmd_snip)
        });
        return;
    }

    let cmd_snip = util::log_snippet(&cmd);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("slider refresh start cmd=\"{}\"", cmd_snip)
    });
    start_refresh(cmd, min, max, step, parse_mode, refresh);
}

fn start_refresh(
    cmd: String,
    min: f64,
    max: f64,
    step: f64,
    parse_mode: NumericParseMode,
    refresh: SliderRefreshState,
) {
    // New refresh id makes older async results stale
    let gen = next_refresh_generation(&refresh.refresh_gen);

    let rx = run_command_capture_status_async(&cmd);
    let refresh_cmd = cmd.clone();
    let refresh_gen = refresh.refresh_gen.clone();
    glib::MainContext::default().spawn_local(async move {
        let output = match rx.recv().await {
            Ok(output) => output,
            Err(_) => {
                // Closed receivers still need to release the gate
                finish_refresh(refresh_cmd, min, max, step, parse_mode, refresh);
                return;
            }
        };
        if refresh_gen.get() == gen {
            match output {
                Ok(output) => {
                    if !output.status.success() {
                        warn!(?cmd, "slider command returned error");
                    } else {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if let Some(value) = parse_numeric(&stdout, min, max, parse_mode) {
                            let muted = parse_muted(&stdout);
                            let formatted = format_value(value);
                            // Skip widget writes when the visible state is already current
                            let value_changed =
                                slider_value_changed(refresh.scale.value(), value, step);
                            let label_changed = refresh.label.text().as_str() != formatted;
                            if value_changed || label_changed {
                                refresh.updating.set(true);
                                if value_changed {
                                    refresh.scale.set_value(value);
                                }
                                if label_changed {
                                    refresh.label.set_text(&formatted);
                                }
                                refresh.updating.set(false);
                                debug::log(PanelDebugLevel::Verbose, || {
                                    format!(
                                        "slider updated cmd=\"{}\" value={value:.1} muted={muted}",
                                        cmd
                                    )
                                });
                            }
                            if let Some(icon_muted) = refresh.icon_muted.as_ref() {
                                // Not every slider has a muted icon pair
                                let icon = if muted {
                                    icon_muted
                                } else {
                                    &refresh.icon_name
                                };
                                refresh.icon_image.set_icon_name(Some(icon));
                            }
                        } else {
                            let snippet = util::log_snippet(stdout.trim());
                            debug::log(PanelDebugLevel::Warn, || {
                                format!(
                                    "slider parse failed cmd=\"{}\" output=\"{}\"",
                                    cmd, snippet
                                )
                            });
                        }
                    }
                }
                Err(err) => {
                    warn!(?err, "slider command failed");
                }
            }
        }

        // Every exit path flows through one gate release
        finish_refresh(refresh_cmd, min, max, step, parse_mode, refresh);
    });
}

fn finish_refresh(
    cmd: String,
    min: f64,
    max: f64,
    step: f64,
    parse_mode: NumericParseMode,
    refresh: SliderRefreshState,
) {
    // One queued refresh is allowed to run after the current one finishes
    if refresh.gate.finish() {
        let cmd_snip = util::log_snippet(&cmd);
        debug::log(PanelDebugLevel::Verbose, || {
            format!(
                "slider refresh consumed pending request cmd=\"{}\"",
                cmd_snip
            )
        });
        request_refresh(cmd, min, max, step, parse_mode, refresh);
    }
}

fn next_refresh_generation(refresh_gen: &Rc<Cell<u64>>) -> u64 {
    // Wrap naturally so stale-result checks stay monotonic enough for UI refresh work
    let next = refresh_gen.get().wrapping_add(1);
    refresh_gen.set(next);
    next
}
