//! CSS provider management and reload orchestration for UnixNotis UIs.

use gtk::gdk;
use gtk::CssProvider;
use unixnotis_core::{
    ThemeConfig, ThemePaths, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS,
};

use super::css_loader::load_provider_with_overrides;
use super::css_overrides::{
    build_base_overrides, build_panel_overrides, build_popup_overrides, build_widgets_overrides,
};

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
            // Base layer is lowest priority so theme overrides can stack above it.
            gtk::style_context_add_provider_for_display(
                &display,
                &self.base,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            if let Some(panel) = self.panel.as_ref() {
                // Panel and popup layers use the same priority to allow overrides.
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
                // Widgets sit on top so card-specific rules can win.
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
        // Base CSS gets the token injection to preserve alpha calculations.
        let base_overrides = build_base_overrides(&self.theme_config);
        load_provider_with_overrides(
            &self.base,
            &self.theme_paths.base_css,
            fallback,
            &base_overrides,
            true,
        );

        if let Some(panel) = self.panel.as_ref() {
            let panel_overrides = build_panel_overrides(&self.theme_config);
            load_provider_with_overrides(
                panel,
                &self.theme_paths.panel_css,
                DEFAULT_PANEL_CSS,
                &panel_overrides,
                false,
            );
        }

        if let Some(widgets) = self.widgets.as_ref() {
            let widgets_overrides = build_widgets_overrides(&self.theme_config);
            load_provider_with_overrides(
                widgets,
                &self.theme_paths.widgets_css,
                DEFAULT_WIDGETS_CSS,
                &widgets_overrides,
                false,
            );
        }

        if let Some(popup) = self.popup.as_ref() {
            let popup_overrides = build_popup_overrides(&self.theme_config);
            load_provider_with_overrides(
                popup,
                &self.theme_paths.popup_css,
                DEFAULT_POPUP_CSS,
                &popup_overrides,
                false,
            );
        }
    }

    pub fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        // Store inputs so the next reload picks up new paths and override settings.
        self.theme_paths = theme_paths;
        self.theme_config = theme_config;
    }
}
