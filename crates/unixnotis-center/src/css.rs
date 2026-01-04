//! CSS loading, validation, and hot-reload support for the center UI.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::CssProvider;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::warn;
use unixnotis_core::{ThemeConfig, ThemePaths, DEFAULT_BASE_CSS, DEFAULT_PANEL_CSS, DEFAULT_WIDGETS_CSS};

use crate::dbus::UiEvent;

pub const DEFAULT_CSS: &str = DEFAULT_BASE_CSS;

/// CSS provider stack for UnixNotis windows.
#[derive(Clone)]
pub struct CssManager {
    theme_paths: ThemePaths,
    theme_config: ThemeConfig,
    panel_width: i32,
    base: CssProvider,
    panel: CssProvider,
    widgets: CssProvider,
    overrides: CssProvider,
}

impl CssManager {
    pub fn new(theme_paths: ThemePaths, theme_config: ThemeConfig, panel_width: i32) -> Self {
        Self {
            theme_paths,
            theme_config,
            panel_width,
            base: CssProvider::new(),
            panel: CssProvider::new(),
            widgets: CssProvider::new(),
            overrides: CssProvider::new(),
        }
    }

    /// Register providers for the default display.
    pub fn apply_to_display(&self) {
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &self.base,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            gtk::style_context_add_provider_for_display(
                &display,
                &self.panel,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
            gtk::style_context_add_provider_for_display(
                &display,
                &self.widgets,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
            );
            gtk::style_context_add_provider_for_display(
                &display,
                &self.overrides,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3,
            );
        }
    }

    /// Reload CSS from disk or fall back to embedded defaults.
    pub fn reload(&self, fallback: &str) {
        load_provider(&self.base, &self.theme_paths.base_css, fallback);
        load_provider(&self.panel, &self.theme_paths.panel_css, DEFAULT_PANEL_CSS);
        load_provider(
            &self.widgets,
            &self.theme_paths.widgets_css,
            DEFAULT_WIDGETS_CSS,
        );
        load_override_provider(&self.overrides, &self.theme_config, self.panel_width);
    }

    pub fn update_theme(
        &mut self,
        theme_paths: ThemePaths,
        theme_config: ThemeConfig,
        panel_width: i32,
    ) {
        self.theme_paths = theme_paths;
        self.theme_config = theme_config;
        self.panel_width = panel_width;
    }
}

/// Start a file watcher for CSS paths and emit UI reload events.
pub fn start_css_watcher(paths: &ThemePaths, sender: async_channel::Sender<UiEvent>) {
    let mut watched_dirs = HashSet::new();
    for path in [&paths.base_css, &paths.panel_css, &paths.widgets_css] {
        if let Some(dir) = path.parent() {
            watched_dirs.insert(dir.to_path_buf());
        }
    }

    if watched_dirs.is_empty() {
        return;
    }

    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = event_tx.send(res);
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                warn!(?err, "failed to create css watcher");
                return;
            }
        };

        for dir in &watched_dirs {
            if let Err(err) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                warn!(?err, "failed to watch css directory");
            }
        }

        let mut last_reload = Instant::now();
        while let Ok(event) = event_rx.recv() {
            if event.is_ok() && last_reload.elapsed() >= Duration::from_millis(150) {
                let _ = sender.try_send(UiEvent::CssReload);
                last_reload = Instant::now();
            }
        }
    });
}

/// Start a file watcher for the config path and emit UI reload events.
pub fn start_config_watcher(config_path: PathBuf, sender: async_channel::Sender<UiEvent>) {
    let Some(parent) = config_path.parent().map(PathBuf::from) else {
        return;
    };
    let config_name = config_path.file_name().map(|name| name.to_os_string());
    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = event_tx.send(res);
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                warn!(?err, "failed to create config watcher");
                return;
            }
        };

        if let Err(err) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
            warn!(?err, "failed to watch config directory");
        }

        let mut last_reload = Instant::now();
        while let Ok(event) = event_rx.recv() {
            let Ok(event) = event else {
                continue;
            };
            if let Some(name) = config_name.as_ref() {
                let matches = event
                    .paths
                    .iter()
                    .any(|path| path.file_name() == Some(name));
                if !matches {
                    continue;
                }
            }
            if last_reload.elapsed() >= Duration::from_millis(150) {
                let _ = sender.try_send(UiEvent::ConfigReload);
                last_reload = Instant::now();
            }
        }
    });
}

fn load_provider(provider: &CssProvider, path: &Path, fallback: &str) {
    match fs::read_to_string(path) {
        Ok(contents) => {
            if contents.trim().is_empty() {
                return;
            }
            provider.load_from_data(&contents);
        }
        Err(_) => provider.load_from_data(fallback),
    }
}

fn load_override_provider(provider: &CssProvider, theme: &ThemeConfig, _panel_width: i32) {
    let border_width = theme.border_width as f32;
    let card_radius = theme.card_radius as f32;
    let card_alpha = theme.card_alpha.clamp(0.0, 1.0);
    let shadow_soft = theme.shadow_soft_alpha.clamp(0.0, 1.0);
    let shadow_strong = theme.shadow_strong_alpha.clamp(0.0, 1.0);
    let css = format!(
        r#"
@define-color unixnotis-shadow-soft alpha(#000000, {shadow_soft});
@define-color unixnotis-shadow-strong alpha(#000000, {shadow_strong});

.unixnotis-panel-card,
.unixnotis-media-card,
.unixnotis-popup-card {{
  border-width: {border_width}px;
  border-style: solid;
  border-radius: {card_radius}px;
}}

.unixnotis-panel-card,
.unixnotis-media-card {{
  background: alpha(@unixnotis-card, {card_alpha});
}}

.unixnotis-popup-card {{
  background: alpha(@unixnotis-surface-strong, {card_alpha});
}}
"#
    );
    provider.load_from_data(&css);
}
