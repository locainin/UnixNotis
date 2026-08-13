//! Popup application startup and GTK runtime ownership

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use glib::MainContext;
use gtk::prelude::*;
use tracing::{info, warn};
use unixnotis_core::Config;
use unixnotis_ui::{
    css::{self, CssKind},
    presentation::register_semantic_badges,
};

use crate::{dbus, ui};

use super::reload::{start_reload_timer, ReloadGate};
use super::runtime::handle_ui_event;
use super::startup::{init_tracing, is_wayland_session, load_config, ConfigSource};

const UI_EVENT_QUEUE_CAPACITY: usize = 512;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// Path to config.toml
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
}

pub fn run(args: Args) -> Result<()> {
    register_semantic_badges().map_err(anyhow::Error::msg)?;
    // Load and validate config before GTK starts so startup failures stay clear
    let (config, config_path, config_source) = load_config(&args).context("load config")?;
    init_tracing(&config);
    let config_source = match config_source {
        ConfigSource::Custom => "custom",
        ConfigSource::Default => "default",
        ConfigSource::Builtin => "builtin",
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
    // Popup startup never creates or migrates user-editable theme files

    let app = gtk::Application::new(Some("com.unixnotis.Popups"), Default::default());
    // Activation can happen more than once in one process, so runtime setup
    // needs one gate that makes repeated activation a no-op
    let activation_started = Rc::new(Cell::new(false));

    app.connect_activate(move |app| {
        // Repeated activation should not start a second D-Bus runtime or watcher set
        if activation_started.replace(true) {
            info!("popup activation ignored because runtime is already initialized");
            return;
        }

        // Bound the queue so a stalled UI cannot grow memory forever
        let (event_tx, event_rx) = async_channel::bounded(UI_EVENT_QUEUE_CAPACITY);
        let dbus_runtime = dbus::start_dbus_runtime(event_tx.clone());
        let command_tx = dbus_runtime.command_sender();
        let shutdown = dbus_runtime.clone();
        app.connect_shutdown(move |_| {
            shutdown.request_shutdown();
        });
        let reload_gate = Arc::new(ReloadGate::new());
        // Timer state keeps only one flush source alive at a time
        let reload_timer = Arc::new(Mutex::new(None::<glib::SourceId>));

        let css_manager = css::CssManager::new_popup(theme_paths.clone(), config.theme.clone());
        let _ = css_manager.apply_to_display();
        let report = css_manager.reload(css::DEFAULT_CSS);
        ui::css_reload::log_reload_failures(&report, "startup");

        let ui = Rc::new(std::cell::RefCell::new(ui::UiState::new(
            app,
            config.clone(),
            config_path.clone(),
            command_tx,
            css_manager,
        )));
        ui.borrow_mut().set_popup_event_sender(event_tx.clone());
        // Composite readiness now means GTK state exists as well as D-Bus seeding succeeding
        dbus_runtime.mark_gtk_ready();

        let ui_clone = ui;
        let reload_gate_loop = Arc::clone(&reload_gate);
        let event_tx_loop = event_tx.clone();
        let reload_timer_loop = Arc::clone(&reload_timer);
        MainContext::default().spawn_local(async move {
            // Closing the channel ends the loop naturally during application shutdown
            while let Ok(event) = event_rx.recv().await {
                handle_ui_event(
                    &ui_clone,
                    &reload_gate_loop,
                    &event_tx_loop,
                    &reload_timer_loop,
                    event,
                );
            }
        });

        if let Err(err) = css::start_css_watcher(&theme_paths, CssKind::Popup, {
            let event_tx = event_tx.clone();
            let reload_gate = Arc::clone(&reload_gate);
            let reload_timer = Arc::clone(&reload_timer);
            move || {
                // Only start the retry timer when queue pressure actually blocked the send
                if reload_gate.request_css(&event_tx) {
                    // GTK source creation returns to the main context from the watcher thread
                    let reload_gate = Arc::clone(&reload_gate);
                    let event_tx = event_tx.clone();
                    let reload_timer = Arc::clone(&reload_timer);
                    MainContext::default().invoke(move || {
                        start_reload_timer(&reload_gate, &event_tx, &reload_timer);
                    });
                }
            }
        }) {
            warn!(?err, "failed to start popup css watcher");
        }
        if let Err(err) = css::start_config_watcher(&config_path, {
            let event_tx = event_tx;
            let reload_gate = Arc::clone(&reload_gate);
            let reload_timer = Arc::clone(&reload_timer);
            move || {
                // Config reloads use the same bounded retry path as popup CSS reloads
                if reload_gate.request_config(&event_tx) {
                    // Config and CSS events share one timer but keep separate pending flags
                    let reload_gate = Arc::clone(&reload_gate);
                    let event_tx = event_tx.clone();
                    let reload_timer = Arc::clone(&reload_timer);
                    MainContext::default().invoke(move || {
                        start_reload_timer(&reload_gate, &event_tx, &reload_timer);
                    });
                }
            }
        }) {
            warn!(?err, "failed to start popup config watcher");
        }
        info!("unixnotis-popups running");
    });

    app.run();
    // GTK returning means every application-owned window and main-loop source has stopped
    Ok(())
}

#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
