//! Center application entrypoint and GTK initialization.

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use glib::MainContext;
use gtk::prelude::*;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;
use unixnotis_ui::css::{self, CssKind};
use zbus::Connection;

mod dbus;
mod debug;
mod media;
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
    info!(config_source, "center configuration loaded");
    if unixnotis_core::util::diagnostic_mode() {
        info!(
            limit = unixnotis_core::util::log_limit(),
            "diagnostic logging enabled (snippets capped; newlines stripped)"
        );
    }

    if !is_wayland_session() {
        return Err(anyhow!(
            "Wayland session not detected; panel UI requires Wayland"
        ));
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

    let app = gtk::Application::new(Some("com.unixnotis.Center"), Default::default());

    app.connect_activate(move |app| {
        let (event_tx, event_rx) = async_channel::unbounded();
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => Arc::new(runtime),
            Err(err) => {
                warn!(?err, "failed to initialize async runtime");
                return;
            }
        };
        let connection = match runtime.block_on(Connection::session()) {
            Ok(connection) => connection,
            Err(err) => {
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
        MainContext::default().spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                ui_clone.borrow_mut().handle_event(event);
            }
        });

        css::start_css_watcher(&theme_paths, CssKind::Panel, {
            let event_tx = event_tx.clone();
            move || {
                let _ = event_tx.try_send(dbus::UiEvent::CssReload);
            }
        });
        css::start_config_watcher(config_path.clone(), move || {
            let _ = event_tx.try_send(dbus::UiEvent::ConfigReload);
        });
        info!("unixnotis-center running");
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
