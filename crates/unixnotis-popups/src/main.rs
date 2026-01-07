//! Popup application entrypoint and GTK initialization.

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use glib::MainContext;
use gtk::prelude::*;
use tracing::info;
use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;
use unixnotis_ui::css::{self, CssKind};

mod dbus;
mod ui;

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

    let theme_base = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| Config::default_config_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let theme_paths = config
        .resolve_theme_paths_from(&theme_base)
        .context("resolve theme paths")?;
    config
        .ensure_theme_files(&theme_paths)
        .context("ensure theme files")?;

    let app = gtk::Application::new(Some("com.unixnotis.Popups"), Default::default());

    app.connect_activate(move |app| {
        let (event_tx, event_rx) = async_channel::unbounded();
        let command_tx = dbus::start_dbus_runtime(event_tx.clone());

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
        MainContext::default().spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                ui_clone.borrow_mut().handle_event(event);
            }
        });

        css::start_css_watcher(&theme_paths, CssKind::Popup, {
            let event_tx = event_tx.clone();
            move || {
                let _ = event_tx.try_send(dbus::UiEvent::CssReload);
            }
        });
        css::start_config_watcher(config_path.clone(), move || {
            let _ = event_tx.try_send(dbus::UiEvent::ConfigReload);
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
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            config
                .general
                .log_level
                .clone()
                .unwrap_or_else(|| "info".to_string()),
        )
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn is_wayland_session() -> bool {
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        if session_type.eq_ignore_ascii_case("wayland") {
            return true;
        }
    }
    env::var("WAYLAND_DISPLAY").is_ok()
}
