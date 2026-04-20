//! Lightweight runtime performance probe for panel open and settled windows
//!
//! This probe is opt-in via `UNIXNOTIS_PERF_PROBE=1` and is designed to answer:
//! - what timers and callbacks stay active after panel settle
//! - how much UI write churn happens in refresh-heavy paths
//! - whether marquee/watch/refresh loops dominate steady-state activity

use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gtk::glib;
use tracing::info;

#[derive(Default)]
struct Counters {
    // Sequence marker for each panel-open window
    panel_open_seq: AtomicU64,
    refresh_timer_armed: AtomicU64,
    refresh_timer_fired: AtomicU64,
    refresh_widgets_calls: AtomicU64,
    refresh_fast_lane_due: AtomicU64,
    refresh_slow_lane_due: AtomicU64,
    refresh_volume_calls: AtomicU64,
    refresh_brightness_calls: AtomicU64,
    refresh_toggles_calls: AtomicU64,
    refresh_stats_calls: AtomicU64,
    refresh_cards_calls: AtomicU64,
    watch_events: AtomicU64,
    slider_refresh_start: AtomicU64,
    slider_refresh_queued: AtomicU64,
    slider_value_writes: AtomicU64,
    slider_label_writes: AtomicU64,
    slider_icon_writes: AtomicU64,
    toggle_refresh_start: AtomicU64,
    toggle_refresh_queued: AtomicU64,
    toggle_state_writes: AtomicU64,
    toggle_class_writes: AtomicU64,
    marquee_start: AtomicU64,
    marquee_stop: AtomicU64,
    marquee_tick: AtomicU64,
    marquee_hold_skip: AtomicU64,
    marquee_label_writes: AtomicU64,
}

impl Counters {
    fn reset(&self) {
        // Keep sequence across windows and reset only per-window activity counters
        self.refresh_timer_armed.store(0, Ordering::Relaxed);
        self.refresh_timer_fired.store(0, Ordering::Relaxed);
        self.refresh_widgets_calls.store(0, Ordering::Relaxed);
        self.refresh_fast_lane_due.store(0, Ordering::Relaxed);
        self.refresh_slow_lane_due.store(0, Ordering::Relaxed);
        self.refresh_volume_calls.store(0, Ordering::Relaxed);
        self.refresh_brightness_calls.store(0, Ordering::Relaxed);
        self.refresh_toggles_calls.store(0, Ordering::Relaxed);
        self.refresh_stats_calls.store(0, Ordering::Relaxed);
        self.refresh_cards_calls.store(0, Ordering::Relaxed);
        self.watch_events.store(0, Ordering::Relaxed);
        self.slider_refresh_start.store(0, Ordering::Relaxed);
        self.slider_refresh_queued.store(0, Ordering::Relaxed);
        self.slider_value_writes.store(0, Ordering::Relaxed);
        self.slider_label_writes.store(0, Ordering::Relaxed);
        self.slider_icon_writes.store(0, Ordering::Relaxed);
        self.toggle_refresh_start.store(0, Ordering::Relaxed);
        self.toggle_refresh_queued.store(0, Ordering::Relaxed);
        self.toggle_state_writes.store(0, Ordering::Relaxed);
        self.toggle_class_writes.store(0, Ordering::Relaxed);
        self.marquee_start.store(0, Ordering::Relaxed);
        self.marquee_stop.store(0, Ordering::Relaxed);
        self.marquee_tick.store(0, Ordering::Relaxed);
        self.marquee_hold_skip.store(0, Ordering::Relaxed);
        self.marquee_label_writes.store(0, Ordering::Relaxed);
    }
}

struct Probe {
    // Env-gated flag is read on each hook to keep probe fully opt-in
    enabled: AtomicBool,
    counters: Counters,
}

impl Probe {
    fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            counters: Counters::default(),
        }
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

static PROBE: OnceLock<Probe> = OnceLock::new();

fn probe() -> &'static Probe {
    PROBE.get_or_init(|| Probe::new(read_enabled_from_env()))
}

fn read_enabled_from_env() -> bool {
    // Accept common truthy forms so shell scripts can toggle probe ergonomically
    matches!(
        env::var("UNIXNOTIS_PERF_PROBE").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    )
}

fn inc(counter: &AtomicU64) {
    // Hook path does a single atomic increment and no formatting/allocation
    if probe().enabled() {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

pub(super) fn on_panel_open(panel_visible_flag: Arc<AtomicBool>) {
    let probe = probe();
    if !probe.enabled() {
        return;
    }
    let seq = probe
        .counters
        .panel_open_seq
        .fetch_add(1, Ordering::Relaxed)
        + 1;
    // Fresh window starts at zero so snapshots are easy to compare across runs
    probe.counters.reset();
    info!(
        panel_open_seq = seq,
        "perf_probe: panel open counters reset"
    );
    schedule_snapshot(
        "open+2s",
        Duration::from_secs(2),
        panel_visible_flag.clone(),
    );
    schedule_snapshot("steady+12s", Duration::from_secs(12), panel_visible_flag);
}

pub(super) fn on_panel_close() {
    if !probe().enabled() {
        return;
    }
    log_snapshot("panel-close");
}

fn schedule_snapshot(label: &'static str, delay: Duration, panel_visible_flag: Arc<AtomicBool>) {
    if !probe().enabled() {
        return;
    }
    glib::timeout_add_local_once(delay, move || {
        // Skip closed panels so delayed logs stay aligned with visible workload
        if panel_visible_flag.load(Ordering::SeqCst) {
            log_snapshot(label);
        }
    });
}

pub(super) fn log_snapshot(label: &str) {
    let probe = probe();
    if !probe.enabled() {
        return;
    }
    // Structured counters keep diffs scriptable across runs and branches
    let c = &probe.counters;
    info!(
        label,
        refresh_timer_armed = c.refresh_timer_armed.load(Ordering::Relaxed),
        refresh_timer_fired = c.refresh_timer_fired.load(Ordering::Relaxed),
        refresh_widgets_calls = c.refresh_widgets_calls.load(Ordering::Relaxed),
        refresh_fast_lane_due = c.refresh_fast_lane_due.load(Ordering::Relaxed),
        refresh_slow_lane_due = c.refresh_slow_lane_due.load(Ordering::Relaxed),
        refresh_volume_calls = c.refresh_volume_calls.load(Ordering::Relaxed),
        refresh_brightness_calls = c.refresh_brightness_calls.load(Ordering::Relaxed),
        refresh_toggles_calls = c.refresh_toggles_calls.load(Ordering::Relaxed),
        refresh_stats_calls = c.refresh_stats_calls.load(Ordering::Relaxed),
        refresh_cards_calls = c.refresh_cards_calls.load(Ordering::Relaxed),
        watch_events = c.watch_events.load(Ordering::Relaxed),
        slider_refresh_start = c.slider_refresh_start.load(Ordering::Relaxed),
        slider_refresh_queued = c.slider_refresh_queued.load(Ordering::Relaxed),
        slider_value_writes = c.slider_value_writes.load(Ordering::Relaxed),
        slider_label_writes = c.slider_label_writes.load(Ordering::Relaxed),
        slider_icon_writes = c.slider_icon_writes.load(Ordering::Relaxed),
        toggle_refresh_start = c.toggle_refresh_start.load(Ordering::Relaxed),
        toggle_refresh_queued = c.toggle_refresh_queued.load(Ordering::Relaxed),
        toggle_state_writes = c.toggle_state_writes.load(Ordering::Relaxed),
        toggle_class_writes = c.toggle_class_writes.load(Ordering::Relaxed),
        marquee_start = c.marquee_start.load(Ordering::Relaxed),
        marquee_stop = c.marquee_stop.load(Ordering::Relaxed),
        marquee_tick = c.marquee_tick.load(Ordering::Relaxed),
        marquee_hold_skip = c.marquee_hold_skip.load(Ordering::Relaxed),
        marquee_label_writes = c.marquee_label_writes.load(Ordering::Relaxed),
        "perf_probe snapshot"
    );
}

// Small wrappers keep call sites readable and keep counter ownership local
pub(in crate::ui) fn refresh_timer_armed() {
    inc(&probe().counters.refresh_timer_armed);
}

pub(in crate::ui) fn refresh_timer_fired() {
    inc(&probe().counters.refresh_timer_fired);
}

pub(in crate::ui) fn refresh_widgets_called() {
    inc(&probe().counters.refresh_widgets_calls);
}

pub(in crate::ui) fn refresh_fast_lane_due() {
    inc(&probe().counters.refresh_fast_lane_due);
}

pub(in crate::ui) fn refresh_slow_lane_due() {
    inc(&probe().counters.refresh_slow_lane_due);
}

pub(in crate::ui) fn refresh_volume_called() {
    inc(&probe().counters.refresh_volume_calls);
}

pub(in crate::ui) fn refresh_brightness_called() {
    inc(&probe().counters.refresh_brightness_calls);
}

pub(in crate::ui) fn refresh_toggles_called() {
    inc(&probe().counters.refresh_toggles_calls);
}

pub(in crate::ui) fn refresh_stats_called() {
    inc(&probe().counters.refresh_stats_calls);
}

pub(in crate::ui) fn refresh_cards_called() {
    inc(&probe().counters.refresh_cards_calls);
}

pub(in crate::ui) fn watch_event() {
    inc(&probe().counters.watch_events);
}

pub(in crate::ui) fn slider_refresh_start() {
    inc(&probe().counters.slider_refresh_start);
}

pub(in crate::ui) fn slider_refresh_queued() {
    inc(&probe().counters.slider_refresh_queued);
}

pub(in crate::ui) fn slider_value_write() {
    inc(&probe().counters.slider_value_writes);
}

pub(in crate::ui) fn slider_label_write() {
    inc(&probe().counters.slider_label_writes);
}

pub(in crate::ui) fn slider_icon_write() {
    inc(&probe().counters.slider_icon_writes);
}

pub(in crate::ui) fn toggle_refresh_start() {
    inc(&probe().counters.toggle_refresh_start);
}

pub(in crate::ui) fn toggle_refresh_queued() {
    inc(&probe().counters.toggle_refresh_queued);
}

pub(in crate::ui) fn toggle_state_write() {
    inc(&probe().counters.toggle_state_writes);
}

pub(in crate::ui) fn toggle_class_write() {
    inc(&probe().counters.toggle_class_writes);
}

pub(in crate::ui) fn marquee_start() {
    inc(&probe().counters.marquee_start);
}

pub(in crate::ui) fn marquee_stop() {
    inc(&probe().counters.marquee_stop);
}

pub(in crate::ui) fn marquee_tick() {
    inc(&probe().counters.marquee_tick);
}

pub(in crate::ui) fn marquee_hold_skip() {
    inc(&probe().counters.marquee_hold_skip);
}

pub(in crate::ui) fn marquee_label_write() {
    inc(&probe().counters.marquee_label_writes);
}
