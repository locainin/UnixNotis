//! GTK application activation and background runtime wiring

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glib::MainContext;
use gtk::prelude::*;
use tracing::{info, warn};
use unixnotis_core::{Config, ThemePaths};
use unixnotis_ui::css::{self, CssKind};

use super::reload::{start_reload_timer, ReloadGate};
use crate::{control, media, ui};

const UI_EVENT_QUEUE_CAPACITY: usize = 512;

pub fn run_center(config: Config, config_path: PathBuf, theme_paths: ThemePaths) {
    let app = gtk::Application::new(Some("com.unixnotis.Center"), Default::default());
    // Activation can fire more than once in one process
    let activation_started = Rc::new(Cell::new(false));

    app.connect_activate(move |app| {
        // Runtime, watcher, and signal-loop ownership is one-shot per process
        if activation_started.replace(true) {
            info!("center activation ignored because runtime is already initialized");
            return;
        }

        let (event_tx, event_rx) = async_channel::bounded(UI_EVENT_QUEUE_CAPACITY);
        let reload_gate = Arc::new(ReloadGate::new());
        let reload_timer = Arc::new(Mutex::new(None::<glib::SourceId>));
        let runtime = if let Some(runtime) = build_runtime() {
            Arc::new(runtime)
        } else {
            // A later GTK activation may retry after transient resource pressure
            activation_started.set(false);
            return;
        };

        // Background runtimes own bus connections so broker restarts do not require a new GTK app
        let command_tx = control::start_control_task(runtime.handle(), event_tx.clone());
        let css_manager = css::CssManager::new_panel(theme_paths.clone(), config.theme.clone());
        let _ = css_manager.apply_to_display();
        let _ = css_manager.reload(css::DEFAULT_CSS);
        let media_handle =
            media::start_media_task(runtime.handle(), config.media.clone(), event_tx.clone());

        let ui = Rc::new(RefCell::new(ui::UiState::new(ui::UiStateInit {
            app: app.clone(),
            config: config.clone(),
            config_path: config_path.clone(),
            command_tx,
            css: css_manager,
            event_tx: event_tx.clone(),
            media_handle,
            runtime,
        })));

        start_ui_event_loop(
            Rc::clone(&ui),
            event_tx.clone(),
            event_rx,
            Arc::clone(&reload_gate),
            Arc::clone(&reload_timer),
        );
        start_watchers(
            &theme_paths,
            &config_path,
            event_tx,
            reload_gate,
            reload_timer,
        );
        info!("unixnotis-center running");
    });

    // GTK receives process arguments while daemon-provided config paths travel through the env
    app.run();
}

fn build_runtime() -> Option<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        // The center is I/O bound, so two workers avoid an oversized idle pool
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|err| {
            warn!(?err, "failed to initialize async runtime");
            err
        })
        .ok()
}

fn start_ui_event_loop(
    ui: Rc<RefCell<ui::UiState>>,
    event_tx: async_channel::Sender<control::UiEvent>,
    event_rx: async_channel::Receiver<control::UiEvent>,
    reload_gate: Arc<ReloadGate>,
    reload_timer: Arc<Mutex<Option<glib::SourceId>>>,
) {
    let rebuild_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    MainContext::default().spawn_local(async move {
        while let Ok(event) = event_rx.recv().await {
            let mut state = ui.borrow_mut();
            let mut needs_retry_timer = handle_event(&mut state, event, &reload_gate, &event_tx);

            // Draining one batch avoids scheduling repeated GTK work for the same burst
            while let Ok(next_event) = event_rx.try_recv() {
                needs_retry_timer |= handle_event(&mut state, next_event, &reload_gate, &event_tx);
            }
            reload_gate.flush(&event_tx);
            if needs_retry_timer || reload_gate.has_pending() {
                start_reload_timer(&reload_gate, &event_tx, &reload_timer);
            }

            // List reconstruction is limited to one visible-panel update per frame
            if state.list_needs_rebuild()
                && state.panel_is_visible()
                && rebuild_source.borrow().is_none()
            {
                let ui_weak = Rc::downgrade(&ui);
                let rebuild_source_handle = Rc::clone(&rebuild_source);
                let source_id =
                    glib::timeout_add_local_once(Duration::from_millis(16), move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut state = ui.borrow_mut();
                            if state.panel_is_visible() {
                                state.flush_list_rebuild();
                            }
                        }
                        *rebuild_source_handle.borrow_mut() = None;
                    });
                *rebuild_source.borrow_mut() = Some(source_id);
            }
        }
    });
}

fn handle_event(
    ui: &mut ui::UiState,
    event: control::UiEvent,
    reload_gate: &ReloadGate,
    event_tx: &async_channel::Sender<control::UiEvent>,
) -> bool {
    let is_css_reload = matches!(&event, control::UiEvent::CssReload);
    let is_config_reload = matches!(&event, control::UiEvent::ConfigReload);
    ui.handle_event(event);
    if is_css_reload {
        reload_gate.complete_css(event_tx)
    } else if is_config_reload {
        reload_gate.complete_config(event_tx)
    } else {
        false
    }
}

fn start_watchers(
    theme_paths: &ThemePaths,
    config_path: &std::path::Path,
    event_tx: async_channel::Sender<control::UiEvent>,
    reload_gate: Arc<ReloadGate>,
    reload_timer: Arc<Mutex<Option<glib::SourceId>>>,
) {
    if let Err(err) = css::start_css_watcher(theme_paths, CssKind::Panel, {
        let event_tx = event_tx.clone();
        let reload_gate = Arc::clone(&reload_gate);
        let reload_timer = Arc::clone(&reload_timer);
        move || request_reload(&reload_gate, &event_tx, &reload_timer, false)
    }) {
        warn!(?err, "failed to start panel css watcher");
    }

    if let Err(err) = css::start_config_watcher(config_path, {
        move || request_reload(&reload_gate, &event_tx, &reload_timer, true)
    }) {
        warn!(?err, "failed to start panel config watcher");
    }
}

fn request_reload(
    reload_gate: &Arc<ReloadGate>,
    event_tx: &async_channel::Sender<control::UiEvent>,
    reload_timer: &Arc<Mutex<Option<glib::SourceId>>>,
    config: bool,
) {
    let needs_retry = if config {
        reload_gate.request_config(event_tx)
    } else {
        reload_gate.request_css(event_tx)
    };
    if !needs_retry {
        return;
    }

    let reload_gate = Arc::clone(reload_gate);
    let event_tx = event_tx.clone();
    let reload_timer = Arc::clone(reload_timer);
    MainContext::default().invoke(move || {
        start_reload_timer(&reload_gate, &event_tx, &reload_timer);
    });
}

#[cfg(test)]
#[path = "tests/app.rs"]
mod tests;
