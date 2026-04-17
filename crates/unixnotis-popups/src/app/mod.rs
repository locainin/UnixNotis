//! Popup startup glue kept outside the crate entrypoint
//!
//! This module keeps the binary `main.rs` small while leaving reload
//! coalescing and GTK runtime wiring in focused files

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use glib::MainContext;
use gtk::prelude::*;
use tracing::info;
use unixnotis_core::Config;
use unixnotis_ui::css::{self, CssKind};

use crate::{dbus, ui};

mod reload;
mod runtime;
mod startup;
#[cfg(test)]
mod tests;

use self::reload::{start_reload_timer, ReloadGate};
use self::runtime::handle_ui_event;
use self::startup::{init_tracing, is_wayland_session, load_config};

const UI_EVENT_QUEUE_CAPACITY: usize = 512;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Args {
    /// Path to config.toml
    #[arg(long)]
    config: Option<PathBuf>,
}

pub(crate) fn run(args: Args) -> Result<()> {
    // Load and validate config before GTK starts so startup failures stay clear
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
        let command_tx = dbus::start_dbus_runtime(event_tx.clone());
        let reload_gate = Arc::new(ReloadGate::new());
        // Timer state keeps only one flush source alive at a time
        let reload_timer = Arc::new(Mutex::new(None::<glib::SourceId>));

        let css_manager = css::CssManager::new_popup(theme_paths.clone(), config.theme.clone());
        css_manager.apply_to_display();
        css_manager.reload(css::DEFAULT_CSS);

        let ui = Rc::new(std::cell::RefCell::new(ui::UiState::new(
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
                handle_ui_event(
                    &ui_clone,
                    &reload_gate_loop,
                    &event_tx_loop,
                    &reload_timer_loop,
                    event,
                );
            }
        });

        css::start_css_watcher(&theme_paths, CssKind::Popup, {
            let event_tx = event_tx.clone();
            let reload_gate = Arc::clone(&reload_gate);
            let reload_timer = Arc::clone(&reload_timer);
            move || {
                // Only start the retry timer when queue pressure actually blocked the send
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
                // Config reloads use the same bounded retry path as popup CSS reloads
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
