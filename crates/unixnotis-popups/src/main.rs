//! Popup application entrypoint and GTK initialization.

#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

use std::cell::{Cell, RefCell};
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use glib::{ControlFlow, MainContext};
use gtk::prelude::*;
use tracing::info;
use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;
use unixnotis_ui::css::{self, CssKind};

mod dbus;
mod ui;

const UI_EVENT_QUEUE_CAPACITY: usize = 512;
const RELOAD_FLUSH_INTERVAL_MS: u64 = 200;

// Coalesces reload requests so config/CSS changes remain eventually consistent
// even when the UI event queue is temporarily full.
struct ReloadGate {
    css: ReloadSlot,
    config: ReloadSlot,
}

// No reload event is represented right now.
const RELOAD_IDLE: u8 = 0;
// Channel capacity blocked a reload send, so the timer must retry it.
const RELOAD_PENDING_RETRY: u8 = 1;
// A reload event is queued or currently being handled on the main loop.
const RELOAD_QUEUED_OR_RUNNING: u8 = 2;

// Each reload kind tracks both the represented reload and whether another
// watcher change arrived after that represented reload was already claimed.
struct ReloadSlot {
    state: AtomicU8,
    dirty_again: AtomicBool,
}

impl ReloadSlot {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(RELOAD_IDLE),
            dirty_again: AtomicBool::new(false),
        }
    }

    fn has_retry_pending(&self) -> bool {
        self.state.load(Ordering::Acquire) == RELOAD_PENDING_RETRY
    }

    fn request(&self, sender: &async_channel::Sender<dbus::UiEvent>, event: dbus::UiEvent) -> bool {
        loop {
            match self.state.load(Ordering::Acquire) {
                RELOAD_IDLE => {
                    // Claim the slot first so only one represented reload exists
                    // for this event kind at a time
                    if self
                        .state
                        .compare_exchange(
                            RELOAD_IDLE,
                            RELOAD_QUEUED_OR_RUNNING,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_err()
                    {
                        continue;
                    }
                    return self.dispatch(sender, event);
                }
                RELOAD_PENDING_RETRY | RELOAD_QUEUED_OR_RUNNING => {
                    // Another watcher hit landed after the represented reload
                    self.dirty_again.store(true, Ordering::Release);
                    return false;
                }
                _ => unreachable!("invalid reload slot state"),
            }
        }
    }

    fn flush(&self, sender: &async_channel::Sender<dbus::UiEvent>, event: dbus::UiEvent) {
        if self.state.load(Ordering::Acquire) != RELOAD_PENDING_RETRY {
            return;
        }

        // A retry that finally enters the queue already covers everything seen
        // up to the point where this send succeeds
        let had_trailing_change = self.dirty_again.swap(false, Ordering::AcqRel);
        match sender.try_send(event) {
            Ok(()) => {
                self.state
                    .store(RELOAD_QUEUED_OR_RUNNING, Ordering::Release);
            }
            Err(async_channel::TrySendError::Full(_)) => {
                if had_trailing_change {
                    self.dirty_again.store(true, Ordering::Release);
                }
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                self.clear();
            }
        }
    }

    fn complete(
        &self,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
    ) -> bool {
        let had_trailing_change = self.dirty_again.swap(false, Ordering::AcqRel);
        if had_trailing_change {
            // Another watcher hit landed while the current reload was in flight
            return self.dispatch(sender, event);
        }

        // Clear the represented slot, then recheck once more so a watcher hit
        // landing in this narrow window still becomes another reload
        self.state.store(RELOAD_IDLE, Ordering::Release);
        if self.dirty_again.swap(false, Ordering::AcqRel) {
            return self.request(sender, event);
        }
        false
    }

    fn dispatch(
        &self,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
    ) -> bool {
        match sender.try_send(event) {
            Ok(()) => {
                self.state
                    .store(RELOAD_QUEUED_OR_RUNNING, Ordering::Release);
                false
            }
            Err(async_channel::TrySendError::Full(_)) => {
                self.state.store(RELOAD_PENDING_RETRY, Ordering::Release);
                true
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                self.clear();
                false
            }
        }
    }

    fn clear(&self) {
        self.state.store(RELOAD_IDLE, Ordering::Release);
        self.dirty_again.store(false, Ordering::Release);
    }
}

impl ReloadGate {
    fn new() -> Self {
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

// Schedules a periodic flush on the GTK main context until all reloads are delivered.
fn start_reload_timer(
    reload_gate: &Arc<ReloadGate>,
    sender: &async_channel::Sender<dbus::UiEvent>,
    timer_state: &Arc<Mutex<Option<glib::SourceId>>>,
) {
    let mut timer_guard = match timer_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
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
                ControlFlow::Continue
            } else {
                let mut timer_guard = match timer_state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                *timer_guard = None;
                ControlFlow::Break
            }
        });
    *timer_guard = Some(source_id);
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to config.toml
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (config, config_path) = load_config(&args).context("load config")?;
    init_tracing(&config);
    let config_source = if args.config.is_some() {
        "custom"
    } else if config_path.exists() {
        "default"
    } else {
        "builtin"
    };
    info!(config_source, "popup configuration loaded");
    if unixnotis_core::util::diagnostic_mode() {
        info!(
            limit = unixnotis_core::util::log_limit(),
            "diagnostic logging enabled (snippets capped; newlines stripped)"
        );
    }

    if !is_wayland_session() {
        return Err(anyhow!("Wayland session not detected; UI requires Wayland"));
    }

    let theme_base = Config::config_dir_for_path(&config_path).context("resolve config dir")?;
    let theme_paths = config
        .resolve_theme_paths_from(&theme_base)
        .context("resolve theme paths")?;
    config
        .ensure_theme_files(&theme_paths)
        .context("ensure theme files")?;

    let app = gtk::Application::new(Some("com.unixnotis.Popups"), Default::default());
    // Activation can be emitted more than once, so startup wiring must be one-time.
    // This guard avoids duplicate runtimes, watchers, and signal loops in one process.
    let activation_started = Rc::new(Cell::new(false));

    app.connect_activate(move |app| {
        // Ignore repeated activations after successful startup wiring.
        if activation_started.replace(true) {
            info!("popup activation ignored because runtime is already initialized");
            return;
        }
        // Bound the UI event queue to avoid unbounded memory growth under stalls.
        let (event_tx, event_rx) = async_channel::bounded(UI_EVENT_QUEUE_CAPACITY);
        let command_tx = dbus::start_dbus_runtime(event_tx.clone());
        let reload_gate = Arc::new(ReloadGate::new());
        // Reload ticks are scheduled only when a reload is pending and the queue is full.
        let reload_timer = Arc::new(Mutex::new(None::<glib::SourceId>));

        let css_manager = css::CssManager::new_popup(theme_paths.clone(), config.theme.clone());
        css_manager.apply_to_display();
        css_manager.reload(css::DEFAULT_CSS);

        let ui = Rc::new(RefCell::new(ui::UiState::new(
            app,
            config.clone(),
            config_path.clone(),
            command_tx,
            css_manager,
        )));

        let ui_clone = ui.clone();
        let reload_gate_loop = Arc::clone(&reload_gate);
        let event_tx_loop = event_tx.clone();
        let reload_timer_loop = Arc::clone(&reload_timer);
        MainContext::default().spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                let is_css_reload = matches!(&event, dbus::UiEvent::CssReload);
                let is_config_reload = matches!(&event, dbus::UiEvent::ConfigReload);
                // Mark reload completion after the handler runs so watcher hits that
                // arrive during handler work still schedule a trailing reload
                ui_clone.borrow_mut().handle_event(event);
                let needs_retry_timer = if is_css_reload {
                    reload_gate_loop.complete_css(&event_tx_loop)
                } else if is_config_reload {
                    reload_gate_loop.complete_config(&event_tx_loop)
                } else {
                    false
                };
                // Attempt to flush pending reloads once a slot becomes available.
                reload_gate_loop.flush(&event_tx_loop);
                if needs_retry_timer || reload_gate_loop.has_pending() {
                    let reload_gate = Arc::clone(&reload_gate_loop);
                    let event_tx = event_tx_loop.clone();
                    let reload_timer = Arc::clone(&reload_timer_loop);
                    MainContext::default().invoke(move || {
                        start_reload_timer(&reload_gate, &event_tx, &reload_timer);
                    });
                }
            }
        });

        css::start_css_watcher(&theme_paths, CssKind::Popup, {
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
        info!("unixnotis-popups running");
    });

    app.run();
    Ok(())
}

fn load_config(args: &Args) -> Result<(Config, PathBuf)> {
    if let Some(path) = args.config.as_ref() {
        return Ok((
            Config::load_from_path(path).context("read config from path")?,
            path.clone(),
        ));
    }
    let path = Config::default_config_path().context("resolve default config path")?;
    let config = Config::load_default().context("read default config")?;
    Ok((config, path))
}

fn init_tracing(config: &Config) {
    let (filter, env_warning) = match EnvFilter::try_from_default_env() {
        Ok(filter) => (filter, None),
        Err(err) => {
            let env_warning = if env::var("RUST_LOG").is_ok() {
                Some(format!(
                    "invalid RUST_LOG value: {err}; falling back to config log_level"
                ))
            } else {
                None
            };
            let configured = config
                .general
                .log_level
                .clone()
                .unwrap_or_else(|| "info".to_string());
            let filter = EnvFilter::try_new(configured.clone()).unwrap_or_else(|err| {
                // Fall back to a safe default instead of crashing on invalid config.
                eprintln!(
                    "unixnotis-popups: invalid log level '{}': {err}; falling back to info",
                    configured
                );
                EnvFilter::new("info")
            });
            (filter, env_warning)
        }
    };
    if let Err(err) = tracing_subscriber::fmt().with_env_filter(filter).try_init() {
        eprintln!("unixnotis-popups: tracing initialization skipped: {err}");
    }
    if let Some(message) = env_warning {
        tracing::warn!("{message}");
    }
}

fn is_wayland_session() -> bool {
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        if session_type.eq_ignore_ascii_case("wayland") {
            return true;
        }
    }
    env::var("WAYLAND_DISPLAY").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_gate_retries_when_queue_is_full() {
        let gate = ReloadGate::new();
        let (tx, rx) = async_channel::bounded(1);

        assert!(!gate.request_css(&tx));
        assert!(!gate.has_pending());

        assert!(gate.request_config(&tx));
        assert!(gate.has_pending());

        let _ = rx.recv_blocking();
        gate.flush(&tx);
        assert!(!gate.has_pending());
    }

    #[test]
    fn reload_gate_keeps_trailing_reload_when_request_arrives_during_handling() {
        let gate = ReloadGate::new();
        let (tx, rx) = async_channel::bounded(1);

        assert!(!gate.request_css(&tx));
        let _ = rx.recv_blocking();

        // Another CSS watcher hit landed while the first reload was still being handled
        assert!(!gate.request_css(&tx));
        assert!(!gate.complete_css(&tx));

        let queued = rx.recv_blocking().expect("queued trailing css reload");
        assert!(matches!(queued, dbus::UiEvent::CssReload));
        assert!(!gate.complete_css(&tx));
        assert!(!gate.has_pending());
    }

    #[test]
    fn reload_gate_does_not_queue_extra_reload_after_retry_covers_latest_state() {
        let gate = ReloadGate::new();
        let (tx, rx) = async_channel::bounded(1);

        assert!(!gate.request_css(&tx));
        assert!(gate.request_config(&tx));
        // The later config change should be covered by the retried config reload
        assert!(!gate.request_config(&tx));

        let _ = rx.recv_blocking();
        gate.flush(&tx);

        let queued = rx.recv_blocking().expect("queued retried config reload");
        assert!(matches!(queued, dbus::UiEvent::ConfigReload));
        assert!(!gate.complete_config(&tx));
        assert!(rx.is_empty());
        assert!(!gate.has_pending());
    }

    #[test]
    fn reload_gate_clears_state_when_queue_is_closed() {
        let gate = ReloadGate::new();
        let (tx, rx) = async_channel::bounded(1);
        drop(rx);

        // Closed queues should clear the slot instead of leaving a stuck pending bit
        assert!(!gate.request_css(&tx));
        assert!(!gate.has_pending());
        assert!(!gate.complete_css(&tx));
        assert!(!gate.has_pending());
    }

    #[test]
    fn reload_gate_tracks_css_and_config_independently() {
        let gate = ReloadGate::new();
        let (tx, rx) = async_channel::bounded(2);

        // A running CSS reload must not block config reloads from being represented too
        assert!(!gate.request_css(&tx));
        assert!(!gate.request_config(&tx));
        assert!(!gate.has_pending());

        let first = rx.recv_blocking().expect("first queued reload");
        let second = rx.recv_blocking().expect("second queued reload");
        assert!(matches!(first, dbus::UiEvent::CssReload));
        assert!(matches!(second, dbus::UiEvent::ConfigReload));

        assert!(!gate.complete_css(&tx));
        assert!(!gate.complete_config(&tx));
        assert!(!gate.has_pending());
    }

    #[test]
    fn reload_gate_keeps_retry_pending_when_new_change_arrives_before_flush() {
        let gate = ReloadGate::new();
        let (tx, rx) = async_channel::bounded(1);

        assert!(!gate.request_css(&tx));
        assert!(gate.request_config(&tx));
        // Another config watcher hit landed while the config reload was still waiting for room
        assert!(!gate.request_config(&tx));
        assert!(gate.has_pending());

        let _ = rx.recv_blocking();
        gate.flush(&tx);

        let queued = rx.recv_blocking().expect("queued pending config reload");
        assert!(matches!(queued, dbus::UiEvent::ConfigReload));
        assert!(!gate.complete_config(&tx));
        assert!(!gate.has_pending());
    }
}
