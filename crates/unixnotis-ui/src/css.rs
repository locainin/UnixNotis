//! CSS loading, validation, and hot-reload support shared by UnixNotis UIs.

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
use unixnotis_core::{
    ThemeConfig, ThemePaths, DEFAULT_BASE_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

pub const DEFAULT_CSS: &str = DEFAULT_BASE_CSS;

/// Identifies which UI surface is loading CSS.
#[derive(Clone, Copy, Debug)]
pub enum CssKind {
    Panel,
    Popup,
}

/// CSS provider stack for UnixNotis windows.
#[derive(Clone)]
pub struct CssManager {
    theme_paths: ThemePaths,
    theme_config: ThemeConfig,
    base: CssProvider,
    panel: Option<CssProvider>,
    widgets: Option<CssProvider>,
    popup: Option<CssProvider>,
}

impl CssManager {
    pub fn new_panel(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            theme_paths,
            theme_config,
            base: CssProvider::new(),
            panel: Some(CssProvider::new()),
            widgets: Some(CssProvider::new()),
            popup: None,
        }
    }

    pub fn new_popup(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            theme_paths,
            theme_config,
            base: CssProvider::new(),
            panel: None,
            widgets: None,
            popup: Some(CssProvider::new()),
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
            if let Some(panel) = self.panel.as_ref() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    panel,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
                );
            }
            if let Some(popup) = self.popup.as_ref() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    popup,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
                );
            }
            if let Some(widgets) = self.widgets.as_ref() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    widgets,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
                );
            }
        }
    }

    /// Reload CSS from disk or fall back to embedded defaults.
    pub fn reload(&self, fallback: &str) {
        let base_overrides = build_base_overrides(&self.theme_config);
        load_provider_with_overrides(
            &self.base,
            &self.theme_paths.base_css,
            fallback,
            &base_overrides,
        );

        if let Some(panel) = self.panel.as_ref() {
            let panel_overrides = build_panel_overrides(&self.theme_config);
            load_provider_with_overrides(
                panel,
                &self.theme_paths.panel_css,
                DEFAULT_PANEL_CSS,
                &panel_overrides,
            );
        }

        if let Some(widgets) = self.widgets.as_ref() {
            let widgets_overrides = build_widgets_overrides(&self.theme_config);
            load_provider_with_overrides(
                widgets,
                &self.theme_paths.widgets_css,
                DEFAULT_WIDGETS_CSS,
                &widgets_overrides,
            );
        }

        if let Some(popup) = self.popup.as_ref() {
            let popup_overrides = build_popup_overrides(&self.theme_config);
            load_provider_with_overrides(
                popup,
                &self.theme_paths.popup_css,
                DEFAULT_POPUP_CSS,
                &popup_overrides,
            );
        }
    }

    pub fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        self.theme_paths = theme_paths;
        self.theme_config = theme_config;
    }
}

/// Start a file watcher for CSS paths and emit reload callbacks.
pub fn start_css_watcher(paths: &ThemePaths, kind: CssKind, on_reload: impl Fn() + Send + 'static) {
    let mut watched_dirs = HashSet::new();
    let css_paths = match kind {
        CssKind::Panel => vec![&paths.base_css, &paths.panel_css, &paths.widgets_css],
        CssKind::Popup => vec![&paths.base_css, &paths.popup_css],
    };
    for path in css_paths {
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
                on_reload();
                last_reload = Instant::now();
            }
        }
    });
}

/// Start a file watcher for the config path and emit reload callbacks.
pub fn start_config_watcher(config_path: PathBuf, on_reload: impl Fn() + Send + 'static) {
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
                on_reload();
                last_reload = Instant::now();
            }
        }
    });
}

fn load_provider_with_overrides(
    provider: &CssProvider,
    path: &Path,
    fallback: &str,
    overrides: &str,
) {
    match fs::read_to_string(path) {
        Ok(contents) => {
            if contents.trim().is_empty() {
                let merged = if overrides.trim().is_empty() {
                    fallback.to_string()
                } else {
                    format!("{fallback}\n{overrides}")
                };
                provider.load_from_data(&merged);
                return;
            }
            let is_default = contents.trim() == fallback.trim();
            let merged = if overrides.trim().is_empty() {
                contents
            } else if is_default {
                format!("{contents}\n{overrides}")
            } else {
                format!("{overrides}\n{contents}")
            };
            provider.load_from_data(&merged);
        }
        Err(_) => {
            if overrides.trim().is_empty() {
                provider.load_from_data(fallback);
                return;
            }
            let merged = format!("{fallback}\n{overrides}");
            provider.load_from_data(&merged);
        }
    }
}

fn build_base_overrides(theme: &ThemeConfig) -> String {
    let surface_alpha = theme.surface_alpha.clamp(0.0, 1.0);
    let surface_strong_alpha = theme.surface_strong_alpha.clamp(0.0, 1.0);
    let shadow_soft = theme.shadow_soft_alpha.clamp(0.0, 1.0);
    let shadow_strong = theme.shadow_strong_alpha.clamp(0.0, 1.0);
    format!(
        r#"
@define-color unixnotis-surface-base @unixnotis-surface;
@define-color unixnotis-surface-strong-base @unixnotis-surface-strong;
@define-color unixnotis-surface alpha(@unixnotis-surface-base, {surface_alpha});
@define-color unixnotis-surface-strong alpha(@unixnotis-surface-strong-base, {surface_strong_alpha});
@define-color unixnotis-shadow-soft alpha(#000000, {shadow_soft});
@define-color unixnotis-shadow-strong alpha(#000000, {shadow_strong});
"#
    )
}

fn build_panel_overrides(theme: &ThemeConfig) -> String {
    let border_width = theme.border_width as f32;
    let card_radius = theme.card_radius as f32;
    let card_alpha = theme.card_alpha.clamp(0.0, 1.0);
    format!(
        r#"
.unixnotis-panel-card {{
  border-width: {border_width}px;
  border-style: solid;
  border-radius: {card_radius}px;
  background: alpha(@unixnotis-card, {card_alpha});
}}
"#
    )
}

fn build_widgets_overrides(theme: &ThemeConfig) -> String {
    let border_width = theme.border_width as f32;
    let card_radius = theme.card_radius as f32;
    let card_alpha = theme.card_alpha.clamp(0.0, 1.0);
    format!(
        r#"
.unixnotis-media-card {{
  border-width: {border_width}px;
  border-style: solid;
  border-radius: {card_radius}px;
  background: alpha(@unixnotis-card, {card_alpha});
}}
"#
    )
}

fn build_popup_overrides(theme: &ThemeConfig) -> String {
    let border_width = theme.border_width as f32;
    let card_radius = theme.card_radius as f32;
    format!(
        r#"
.unixnotis-popup-card {{
  border-width: {border_width}px;
  border-style: solid;
  border-radius: {card_radius}px;
}}
"#
    )
}
