//! Command-backed slider widget entry point

// Keep this file as the public widget shell
// Behavior lives in named helper files so this folder stays navigable
mod actions;
mod apply;
mod build;
mod gate;
#[cfg(test)]
#[path = "tests/gate.rs"]
mod gate_tests;
mod layout;
#[cfg(test)]
#[path = "tests/layout.rs"]
mod layout_tests;
mod poll;
mod refresh;
mod request;
mod schedule;
mod state;
mod value;
#[cfg(test)]
#[path = "tests/value.rs"]
mod value_tests;
mod watching;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use unixnotis_core::SliderWidgetConfig;

use self::build::build_slider_widgets;
use self::gate::SliderRefreshGate;
use self::refresh::request_refresh;
use self::request::SliderRefreshRequest;
use self::state::{SliderRefreshMeta, SliderRefreshState};
use super::{CommandWatch, RefreshBackoff};

pub struct CommandSlider {
    // Root widget embedded by higher-level widget wrappers
    pub root: gtk::Box,
    // Interactive value control
    scale: gtk::Scale,
    // Text label for the current slider value
    value_label: gtk::Label,
    // Icon image is reused by both clickable and static slider variants
    icon_image: gtk::Image,
    // Default icon shown when slider is not muted
    icon_name: String,
    // Optional icon variant for muted state
    icon_muted: Option<String>,
    // Config is retained for refresh and watch lifecycle operations
    config: SliderWidgetConfig,
    // Guard blocks recursive value-changed signals during internal updates
    updating: Rc<Cell<bool>>,
    // Generation token avoids stale async refresh races
    refresh_gen: Rc<Cell<u64>>,
    // Local gate keeps refresh bursts bounded
    refresh_gate: SliderRefreshGate,
    // Stable poll results back off so unchanged sliders do not drive panel wakeups
    refresh_backoff: Rc<RefCell<RefreshBackoff>>,
    // Optional watch command handle for event-driven refresh
    watch_handle: RefCell<Option<CommandWatch>>,
}

impl CommandSlider {
    pub fn new(config: SliderWidgetConfig, extra_class: &str) -> Self {
        // GTK layout construction stays in one place so signal wiring stays readable
        let widgets = build_slider_widgets(&config, extra_class);
        let updating = Rc::new(Cell::new(false));
        // Refresh generation only lives on the GTK main loop
        // Rc<Cell<_>> keeps this path lighter than Arc<AtomicU64>
        let refresh_gen = Rc::new(Cell::new(0_u64));
        let refresh_gate = SliderRefreshGate::new();
        // Backoff is shared by direct refresh calls, watch callbacks, and scheduler due checks
        let refresh_backoff = Rc::new(RefCell::new(RefreshBackoff::default()));
        let refresh_meta = SliderRefreshMeta {
            updating: updating.clone(),
            refresh_gen: refresh_gen.clone(),
            icon_name: widgets.icon_name.clone(),
            icon_muted: widgets.icon_muted.clone(),
            gate: refresh_gate.clone(),
            backoff: refresh_backoff.clone(),
        };

        actions::attach_icon_action(
            &widgets.root,
            &widgets.icon_image,
            &widgets.scale,
            &widgets.value_label,
            &config,
            &refresh_meta,
        );
        // Scale changes and icon clicks both feed the same refresh state
        // That prevents action failures from leaving stale values on screen
        actions::attach_scale_action(
            &widgets.scale,
            &widgets.value_label,
            &widgets.icon_image,
            &config,
            &refresh_meta,
        );

        Self {
            root: widgets.root,
            scale: widgets.scale,
            value_label: widgets.value_label,
            icon_image: widgets.icon_image,
            icon_name: widgets.icon_name,
            icon_muted: widgets.icon_muted,
            config,
            updating,
            refresh_gen,
            refresh_gate,
            refresh_backoff,
            watch_handle: RefCell::new(None),
        }
    }

    pub fn refresh(&self, base_interval: Duration, force: bool) {
        // Public refresh path delegates to shared async fetch routine
        // Background polling respects backoff while forced UI reconciliation stays immediate
        request_refresh(
            SliderRefreshRequest::from_config(&self.config),
            self.refresh_state(),
            base_interval,
            force,
        );
    }

    pub fn next_poll_in(&self, now: Instant, base_interval: Duration) -> Option<Duration> {
        // Scheduler asks each slider for its own deadline instead of using one fast global tick
        poll::next_poll_in(
            &self.watch_handle,
            &self.refresh_gate,
            &self.refresh_backoff,
            now,
            base_interval,
        )
    }

    pub fn set_watch_active(&self, active: bool) {
        // Panel visibility owns watch lifecycle so hidden panels do not keep monitor commands alive
        watching::set_watch_active(self, active);
    }

    fn refresh_state(&self) -> SliderRefreshState {
        // Build one refresh bundle so call sites stay small
        SliderRefreshState {
            scale: self.scale.clone(),
            label: self.value_label.clone(),
            icon_image: self.icon_image.clone(),
            updating: self.updating.clone(),
            refresh_gen: self.refresh_gen.clone(),
            icon_name: self.icon_name.clone(),
            icon_muted: self.icon_muted.clone(),
            gate: self.refresh_gate.clone(),
            backoff: self.refresh_backoff.clone(),
        }
    }
}
