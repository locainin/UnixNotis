//! GTK activation and runtime wiring for the center process

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glib::MainContext;
use gtk::prelude::*;
use tracing::{info, warn};
use unixnotis_core::{Config, ThemePaths};
use unixnotis_ui::css::{self, CssKind};
use zbus::Connection;

use crate::dbus;
use crate::media;
use crate::ui;

const UI_EVENT_QUEUE_CAPACITY: usize = 512;
const RELOAD_FLUSH_INTERVAL_MS: u64 = 200;

// Coalesces reload requests so CSS/config edits are retried when the UI queue is full
struct ReloadGate {
    css_pending: AtomicBool,
    config_pending: AtomicBool,
}

impl ReloadGate {
    fn new() -> Self {
        Self {
            css_pending: AtomicBool::new(false),
            config_pending: AtomicBool::new(false),
        }
    }

    fn request_css(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.request(sender, dbus::UiEvent::CssReload, &self.css_pending)
    }

    fn request_config(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.request(sender, dbus::UiEvent::ConfigReload, &self.config_pending)
    }

    fn flush(&self, sender: &async_channel::Sender<dbus::UiEvent>) {
        self.flush_one(sender, dbus::UiEvent::CssReload, &self.css_pending);
        self.flush_one(sender, dbus::UiEvent::ConfigReload, &self.config_pending);
    }

    fn has_pending(&self) -> bool {
        self.css_pending.load(Ordering::Acquire) || self.config_pending.load(Ordering::Acquire)
    }

    fn request(
        &self,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
        pending: &AtomicBool,
    ) -> bool {
        // One bit tracks whether this reload kind still needs another queue attempt
        if pending.swap(true, Ordering::AcqRel) {
            return false;
        }

        match sender.try_send(event) {
            Ok(()) => {
                pending.store(false, Ordering::Release);
                false
            }
            Err(async_channel::TrySendError::Full(_)) => true,
            Err(async_channel::TrySendError::Closed(_)) => {
                pending.store(false, Ordering::Release);
                false
            }
        }
    }

    fn flush_one(
        &self,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
        pending: &AtomicBool,
    ) {
        if !pending.load(Ordering::Acquire) {
            return;
        }

        match sender.try_send(event) {
            Ok(()) => {
                pending.store(false, Ordering::Release);
            }
            Err(async_channel::TrySendError::Full(_)) => {}
            Err(async_channel::TrySendError::Closed(_)) => {
                pending.store(false, Ordering::Release);
            }
        }
    }
}

fn start_reload_timer(
    reload_gate: &Arc<ReloadGate>,
    sender: &async_channel::Sender<dbus::UiEvent>,
    timer_state: &Arc<Mutex<Option<glib::SourceId>>>,
) {
    let mut timer_guard = match timer_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    // One timer is enough because the gate already tracks both reload kinds
    if timer_guard.is_some() {
        return;
    }

    let reload_gate = Arc::clone(reload_gate);
    let sender = sender.clone();
    let timer_state = Arc::clone(timer_state);
    let source_id =
        glib::timeout_add_local(Duration::from_millis(RELOAD_FLUSH_INTERVAL_MS), move || {
            reload_gate.flush(&sender);
            if reload_gate.has_pending() {
                glib::ControlFlow::Continue
            } else {
                let mut timer_guard = match timer_state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                *timer_guard = None;
                glib::ControlFlow::Break
            }
        });
    *timer_guard = Some(source_id);
}

pub(crate) fn run_center(config: Config, config_path: PathBuf, theme_paths: ThemePaths) {
    let app = gtk::Application::new(Some("com.unixnotis.Center"), Default::default());

    // Activation can fire more than once in one process
    // This guard keeps runtimes, watchers, and signal loops one-shot
    let activation_started = Rc::new(Cell::new(false));

    app.connect_activate(move |app| {
        // Ignore later activate calls after startup wiring already succeeded
        if activation_started.replace(true) {
            info!("center activation ignored because runtime is already initialized");
            return;
        }

        // Bound the UI queue so bursts cannot grow memory without limit
        let (event_tx, event_rx) = async_channel::bounded(UI_EVENT_QUEUE_CAPACITY);
        let reload_gate = Arc::new(ReloadGate::new());
        let reload_timer = Arc::new(Mutex::new(None::<glib::SourceId>));

        let runtime = match tokio::runtime::Builder::new_multi_thread()
            // The center workload is mostly I/O bound
            // Two workers are enough without paying for a larger idle pool
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(runtime) => Arc::new(runtime),
            Err(err) => {
                // Reset the guard so a later activate can retry startup
                activation_started.set(false);
                warn!(?err, "failed to initialize async runtime");
                return;
            }
        };

        let connection = match runtime.block_on(Connection::session()) {
            Ok(connection) => connection,
            Err(err) => {
                // Reset the guard so a later activate can retry startup
                activation_started.set(false);
                warn!(?err, "failed to connect to session bus");
                return;
            }
        };

        let command_tx =
            dbus::start_dbus_task(runtime.handle(), connection.clone(), event_tx.clone());

        let css_manager = css::CssManager::new_panel(theme_paths.clone(), config.theme.clone());
        css_manager.apply_to_display();
        css_manager.reload(css::DEFAULT_CSS);

        let media_handle = media::start_media_task(
            runtime.handle(),
            connection.clone(),
            config.media.clone(),
            event_tx.clone(),
        );

        let ui = Rc::new(RefCell::new(ui::UiState::new(ui::UiStateInit {
            app: app.clone(),
            config: config.clone(),
            config_path: config_path.clone(),
            command_tx,
            css: css_manager,
            event_tx: event_tx.clone(),
            media_handle,
            runtime: runtime.clone(),
        })));

        let ui_clone = ui.clone();
        let reload_gate_loop = Arc::clone(&reload_gate);
        let event_tx_loop = event_tx.clone();
        let rebuild_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        MainContext::default().spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                let mut ui = ui_clone.borrow_mut();
                ui.handle_event(event);

                // Drain the queue in batches so bursts do not schedule extra GTK work
                while let Ok(next_event) = event_rx.try_recv() {
                    ui.handle_event(next_event);
                }

                reload_gate_loop.flush(&event_tx_loop);

                // Rebuild at most once per frame
                // Hidden panels keep the rebuild deferred until the next open
                if ui.list_needs_rebuild()
                    && ui.panel_is_visible()
                    && rebuild_source.borrow().is_none()
                {
                    let ui_weak = Rc::downgrade(&ui_clone);
                    let rebuild_source_handle = rebuild_source.clone();
                    let source_id =
                        glib::timeout_add_local_once(Duration::from_millis(16), move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut ui = ui.borrow_mut();
                                if ui.panel_is_visible() {
                                    ui.flush_list_rebuild();
                                }
                            }
                            *rebuild_source_handle.borrow_mut() = None;
                        });
                    *rebuild_source.borrow_mut() = Some(source_id);
                }
            }
        });

        css::start_css_watcher(&theme_paths, CssKind::Panel, {
            let event_tx = event_tx.clone();
            let reload_gate = Arc::clone(&reload_gate);
            let reload_timer = Arc::clone(&reload_timer);
            move || {
                if reload_gate.request_css(&event_tx) {
                    let reload_gate = Arc::clone(&reload_gate);
                    let event_tx = event_tx.clone();
                    let reload_timer = Arc::clone(&reload_timer);
                    MainContext::default().invoke(move || {
                        start_reload_timer(&reload_gate, &event_tx, &reload_timer);
                    });
                }
            }
        });

        css::start_config_watcher(config_path.clone(), {
            let event_tx = event_tx.clone();
            let reload_gate = Arc::clone(&reload_gate);
            let reload_timer = Arc::clone(&reload_timer);
            move || {
                if reload_gate.request_config(&event_tx) {
                    let reload_gate = Arc::clone(&reload_gate);
                    let event_tx = event_tx.clone();
                    let reload_timer = Arc::clone(&reload_timer);
                    MainContext::default().invoke(move || {
                        start_reload_timer(&reload_gate, &event_tx, &reload_timer);
                    });
                }
            }
        });

        info!("unixnotis-center running");
    });

    app.run();
}
