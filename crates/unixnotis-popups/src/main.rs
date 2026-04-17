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
use std::sync::atomic::{AtomicBool, Ordering};
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
        // Coalesce reloads so a single request represents the latest change.
        if pending.swap(true, Ordering::AcqRel) {
            return false;
        }
        match sender.try_send(event) {
            Ok(()) => {
                pending.store(false, Ordering::Release);
                false
            }
            Err(async_channel::TrySendError::Full(_)) => {
                // Pending flag remains set to retry once capacity is available.
                true
            }
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
        MainContext::default().spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                ui_clone.borrow_mut().handle_event(event);
                // Attempt to flush pending reloads once a slot becomes available.
                reload_gate_loop.flush(&event_tx_loop);
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
    tracing_subscriber::fmt().with_env_filter(filter).init();
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
    fn reload_gate_coalesces_when_queue_is_full() {
        let gate = ReloadGate::new();
        let (tx, rx) = async_channel::bounded(1);

        assert!(!gate.request_css(&tx));
        assert!(!gate.has_pending());

        assert!(gate.request_css(&tx));
        assert!(gate.has_pending());

        let _ = rx.recv_blocking();
        gate.flush(&tx);
        assert!(!gate.has_pending());
    }

    #[test]
    fn reload_gate_skips_duplicate_pending_requests() {
        let gate = ReloadGate::new();
        let (tx, _rx) = async_channel::bounded(1);

        assert!(!gate.request_css(&tx));
        assert!(gate.request_config(&tx));
        assert!(!gate.request_config(&tx));
        assert!(gate.has_pending());
    }
}
