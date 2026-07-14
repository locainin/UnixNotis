//! GTK activation and runtime wiring for the center process

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
use zbus::Connection;

use crate::dbus;
use crate::media;
use crate::ui;

const UI_EVENT_QUEUE_CAPACITY: usize = 512;
const RELOAD_FLUSH_INTERVAL_MS: u64 = 200;

// Coalesces reload requests so CSS/config edits are retried when the UI queue is full
struct ReloadGate {
    css: ReloadSlot,
    config: ReloadSlot,
}

struct ReloadSlot {
    state: Mutex<ReloadSlotState>,
}

#[derive(Default)]
struct ReloadSlotState {
    represented: bool,
    retry_pending: bool,
    dirty_again: bool,
}

impl ReloadSlot {
    const fn new() -> Self {
        Self {
            state: Mutex::new(ReloadSlotState {
                represented: false,
                retry_pending: false,
                dirty_again: false,
            }),
        }
    }

    fn request(&self, sender: &async_channel::Sender<dbus::UiEvent>, event: dbus::UiEvent) -> bool {
        let mut state = self.lock_state();
        let needs_retry = if state.represented {
            // Preserve one trailing reload when a change lands during processing
            state.dirty_again = true;
            false
        } else {
            state.represented = true;
            Self::dispatch(&mut state, sender, event)
        };
        drop(state);
        needs_retry
    }

    fn dispatch(
        state: &mut ReloadSlotState,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
    ) -> bool {
        match sender.try_send(event) {
            Ok(()) => {
                state.retry_pending = false;
                false
            }
            Err(async_channel::TrySendError::Full(_)) => {
                state.retry_pending = true;
                true
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                *state = ReloadSlotState::default();
                false
            }
        }
    }

    fn flush(&self, sender: &async_channel::Sender<dbus::UiEvent>, event: dbus::UiEvent) {
        let mut state = self.lock_state();
        if state.retry_pending {
            // A successful retry covers every change observed before it entered the queue
            let had_trailing_change = std::mem::take(&mut state.dirty_again);
            let _needs_retry = Self::dispatch(&mut state, sender, event);
            if state.retry_pending && had_trailing_change {
                state.dirty_again = true;
            }
        }
        drop(state);
    }

    fn complete(
        &self,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
    ) -> bool {
        let mut state = self.lock_state();
        let needs_retry = if std::mem::take(&mut state.dirty_again) {
            Self::dispatch(&mut state, sender, event)
        } else {
            state.represented = false;
            false
        };
        drop(state);
        needs_retry
    }

    fn has_retry_pending(&self) -> bool {
        let state = self.lock_state();
        let retry_pending = state.retry_pending;
        drop(state);
        retry_pending
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ReloadSlotState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl ReloadGate {
    const fn new() -> Self {
        Self {
            css: ReloadSlot::new(),
            config: ReloadSlot::new(),
        }
    }

    fn request_css(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.css.request(sender, dbus::UiEvent::CssReload)
    }

    fn request_config(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.config.request(sender, dbus::UiEvent::ConfigReload)
    }

    fn flush(&self, sender: &async_channel::Sender<dbus::UiEvent>) {
        self.css.flush(sender, dbus::UiEvent::CssReload);
        self.config.flush(sender, dbus::UiEvent::ConfigReload);
    }

    fn has_pending(&self) -> bool {
        self.css.has_retry_pending() || self.config.has_retry_pending()
    }

    fn complete_css(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.css.complete(sender, dbus::UiEvent::CssReload)
    }

    fn complete_config(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.config.complete(sender, dbus::UiEvent::ConfigReload)
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

pub fn run_center(config: Config, config_path: PathBuf, theme_paths: ThemePaths) {
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
        let _ = css_manager.apply_to_display();
        let _ = css_manager.reload(css::DEFAULT_CSS);

        let media_handle = media::start_media_task(
            runtime.handle(),
            connection,
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
            runtime,
        })));

        let ui_clone = ui;
        let reload_gate_loop = Arc::clone(&reload_gate);
        let reload_timer_loop = Arc::clone(&reload_timer);
        let event_tx_loop = event_tx.clone();
        let rebuild_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        MainContext::default().spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                let mut ui = ui_clone.borrow_mut();
                let is_css_reload = matches!(&event, dbus::UiEvent::CssReload);
                let is_config_reload = matches!(&event, dbus::UiEvent::ConfigReload);
                ui.handle_event(event);
                let mut needs_retry_timer = if is_css_reload {
                    reload_gate_loop.complete_css(&event_tx_loop)
                } else if is_config_reload {
                    reload_gate_loop.complete_config(&event_tx_loop)
                } else {
                    false
                };

                // Drain the queue in batches so bursts do not schedule extra GTK work
                while let Ok(next_event) = event_rx.try_recv() {
                    let is_css_reload = matches!(&next_event, dbus::UiEvent::CssReload);
                    let is_config_reload = matches!(&next_event, dbus::UiEvent::ConfigReload);
                    ui.handle_event(next_event);
                    needs_retry_timer |= if is_css_reload {
                        reload_gate_loop.complete_css(&event_tx_loop)
                    } else if is_config_reload {
                        reload_gate_loop.complete_config(&event_tx_loop)
                    } else {
                        false
                    };
                }

                reload_gate_loop.flush(&event_tx_loop);
                if needs_retry_timer || reload_gate_loop.has_pending() {
                    start_reload_timer(&reload_gate_loop, &event_tx_loop, &reload_timer_loop);
                }

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

        if let Err(err) = css::start_css_watcher(&theme_paths, CssKind::Panel, {
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
        }) {
            warn!(?err, "failed to start panel css watcher");
        }

        if let Err(err) = css::start_config_watcher(&config_path, {
            let event_tx = event_tx;
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
        }) {
            warn!(?err, "failed to start panel config watcher");
        }

        info!("unixnotis-center running");
    });

    // GTK can use the real process argv here because daemon-launched config paths now travel by env
    app.run();
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod tests;
