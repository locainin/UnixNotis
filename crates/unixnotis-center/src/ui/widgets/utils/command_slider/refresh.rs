//! Slider async refresh execution

use std::cell::Cell;
use std::io;
use std::process::Output;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tracing::warn;
use unixnotis_core::{util, PanelDebugLevel};

use super::super::run_command_capture_status_async;
use super::apply::{apply_successful_output, note_slider_error};
use super::request::SliderRefreshRequest;
use super::state::SliderRefreshState;
use crate::debug;
use crate::ui::perf_probe;

pub(super) fn request_refresh(
    request: SliderRefreshRequest,
    refresh: SliderRefreshState,
    base_interval: Duration,
    force: bool,
) {
    if !refresh
        .backoff
        .borrow()
        .should_refresh(Instant::now(), force)
    {
        return;
    }

    // Collapse bursty requests into one running refresh and one trailing refresh
    if !refresh.gate.begin_or_queue() {
        perf_probe::slider_refresh_queued();
        let cmd_snip = util::log_snippet(&request.cmd);
        debug::log(PanelDebugLevel::Verbose, || {
            format!("slider refresh queued while in flight cmd=\"{}\"", cmd_snip)
        });
        return;
    }

    perf_probe::slider_refresh_start();
    let cmd_snip = util::log_snippet(&request.cmd);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("slider refresh start cmd=\"{}\"", cmd_snip)
    });
    start_refresh(request, refresh, base_interval);
}

fn start_refresh(
    request: SliderRefreshRequest,
    refresh: SliderRefreshState,
    base_interval: Duration,
) {
    // New refresh id makes older async results stale
    let gen = next_refresh_generation(&refresh.refresh_gen);
    let rx = run_command_capture_status_async(&request.cmd);
    let refresh_gen = refresh.refresh_gen.clone();

    glib::MainContext::default().spawn_local(async move {
        match rx.recv().await {
            Ok(output) if refresh_gen.get() == gen => {
                handle_worker_result(&request, &refresh, output, base_interval);
            }
            Ok(_) => {
                // Newer refresh work already won the race
            }
            Err(_) => {
                // Closed receivers still need to release the gate
                note_slider_error(&refresh, base_interval);
            }
        }

        // Every exit path flows through one gate release
        finish_refresh(request, refresh, base_interval);
    });
}

fn handle_worker_result(
    request: &SliderRefreshRequest,
    refresh: &SliderRefreshState,
    output: io::Result<Output>,
    base_interval: Duration,
) {
    match output {
        Ok(output) if output.status.success() => {
            apply_successful_output(request, refresh, &output.stdout, base_interval);
        }
        Ok(_) => {
            warn!(cmd = ?request.cmd, "slider command returned error");
            note_slider_error(refresh, base_interval);
        }
        Err(err) => {
            warn!(?err, "slider command failed");
            note_slider_error(refresh, base_interval);
        }
    }
}

fn finish_refresh(
    request: SliderRefreshRequest,
    refresh: SliderRefreshState,
    base_interval: Duration,
) {
    // One queued refresh is allowed to run after the current one finishes
    if refresh.gate.finish() {
        let cmd_snip = util::log_snippet(&request.cmd);
        debug::log(PanelDebugLevel::Verbose, || {
            format!(
                "slider refresh consumed pending request cmd=\"{}\"",
                cmd_snip
            )
        });
        request_refresh(request, refresh, base_interval, true);
    }
}

fn next_refresh_generation(refresh_gen: &Rc<Cell<u64>>) -> u64 {
    // Wrap naturally so stale-result checks stay monotonic enough for UI refresh work
    let next = refresh_gen.get().wrapping_add(1);
    refresh_gen.set(next);
    next
}
