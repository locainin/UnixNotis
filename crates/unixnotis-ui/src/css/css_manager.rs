//! CSS provider management and reload orchestration for UnixNotis UIs.

use gtk::gdk;
use gtk::CssProvider;
use unixnotis_core::{
    ThemeConfig, ThemePaths, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

use super::css_loader::load_provider_with_overrides;
use super::css_overrides::{
    build_base_overrides, build_panel_overrides, build_popup_overrides, build_widgets_overrides,
};

/// Tiny provider boundary so reload behavior can be tested without GTK.
trait CssProviderBackend: Clone {
    fn load_css_data(&self, data: &str);
    fn add_to_display(&self, display: &gdk::Display, priority: u32);
}

impl CssProviderBackend for CssProvider {
    fn load_css_data(&self, data: &str) {
        self.load_from_data(data);
    }

    fn add_to_display(&self, display: &gdk::Display, priority: u32) {
        gtk::style_context_add_provider_for_display(display, self, priority);
    }
}

/// Identifies which UI surface is loading CSS.
#[derive(Clone, Copy, Debug)]
pub enum CssKind {
    Panel,
    Popup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CssProviderLayer {
    Base,
    Panel,
    Popup,
    Widgets,
    Media,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CssProviderRegistration {
    layer: CssProviderLayer,
    priority: u32,
}

/// CSS provider stack for UnixNotis windows.
#[derive(Clone)]
pub struct CssManager {
    inner: CssManagerInner<CssProvider>,
}

impl CssManager {
    pub fn new_panel(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            inner: CssManagerInner::new_panel(theme_paths, theme_config),
        }
    }

    pub fn new_popup(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            inner: CssManagerInner::new_popup(theme_paths, theme_config),
        }
    }

    /// Register providers for the default display.
    pub fn apply_to_display(&self) {
        // Public callers only need the side effect; tests assert the returned internal plan
        let _ = self.inner.apply_to_display();
    }

    /// Reload CSS from disk or fall back to embedded defaults.
    pub fn reload(&self, fallback: &str) {
        // Public callers only need provider reloads; tests assert the returned internal layer list
        let _ = self.inner.reload(fallback);
    }

    pub fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        self.inner.update_theme(theme_paths, theme_config);
    }
}

#[derive(Clone)]
struct CssManagerInner<P>
where
    P: CssProviderBackend,
{
    theme_paths: ThemePaths,
    theme_config: ThemeConfig,
    base: P,
    panel: Option<P>,
    widgets: Option<P>,
    media: Option<P>,
    popup: Option<P>,
}

impl CssManagerInner<CssProvider> {
    fn new_panel(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            theme_paths,
            theme_config,
            base: CssProvider::new(),
            panel: Some(CssProvider::new()),
            widgets: Some(CssProvider::new()),
            media: Some(CssProvider::new()),
            popup: None,
        }
    }

    fn new_popup(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            theme_paths,
            theme_config,
            base: CssProvider::new(),
            panel: None,
            widgets: None,
            media: None,
            popup: Some(CssProvider::new()),
        }
    }
}

impl<P> CssManagerInner<P>
where
    P: CssProviderBackend,
{
    /// Register providers for the default display and return the attempted layer plan.
    fn apply_to_display(&self) -> Vec<CssProviderRegistration> {
        let registrations = self.provider_registrations();
        if let Some(display) = gdk::Display::default() {
            for registration in &registrations {
                if let Some(provider) = self.provider_for_layer(registration.layer) {
                    provider.add_to_display(&display, registration.priority);
                }
            }
        }
        registrations
    }

    /// Reload CSS from disk or fall back to embedded defaults.
    fn reload(&self, fallback: &str) -> Vec<CssProviderLayer> {
        let mut loaded = Vec::new();
        // Base CSS gets the token injection to preserve alpha calculations.
        let base_overrides = build_base_overrides(&self.theme_config);
        load_provider_with_overrides(
            |data| self.base.load_css_data(data),
            &self.theme_paths.base_css,
            fallback,
            &base_overrides,
            true,
        );
        loaded.push(CssProviderLayer::Base);

        if let Some(panel) = self.panel.as_ref() {
            let panel_overrides = build_panel_overrides(&self.theme_config);
            load_provider_with_overrides(
                |data| panel.load_css_data(data),
                &self.theme_paths.panel_css,
                DEFAULT_PANEL_CSS,
                &panel_overrides,
                false,
            );
            loaded.push(CssProviderLayer::Panel);
        }

        if let Some(widgets) = self.widgets.as_ref() {
            let widgets_overrides = build_widgets_overrides(&self.theme_config);
            load_provider_with_overrides(
                |data| widgets.load_css_data(data),
                &self.theme_paths.widgets_css,
                DEFAULT_WIDGETS_CSS,
                &widgets_overrides,
                false,
            );
            loaded.push(CssProviderLayer::Widgets);
        }

        if let Some(media) = self.media.as_ref() {
            // Media css is intentionally isolated so ricing one widget does not pollute widgets.css.
            load_provider_with_overrides(
                |data| media.load_css_data(data),
                &self.theme_paths.media_css,
                DEFAULT_MEDIA_CSS,
                "",
                false,
            );
            loaded.push(CssProviderLayer::Media);
        }

        if let Some(popup) = self.popup.as_ref() {
            let popup_overrides = build_popup_overrides(&self.theme_config);
            load_provider_with_overrides(
                |data| popup.load_css_data(data),
                &self.theme_paths.popup_css,
                DEFAULT_POPUP_CSS,
                &popup_overrides,
                false,
            );
            loaded.push(CssProviderLayer::Popup);
        }
        loaded
    }

    fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        // Store inputs so the next reload picks up new paths and override settings.
        self.theme_paths = theme_paths;
        self.theme_config = theme_config;
    }

    fn provider_registrations(&self) -> Vec<CssProviderRegistration> {
        let mut registrations = vec![CssProviderRegistration {
            layer: CssProviderLayer::Base,
            priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        }];
        if self.panel.is_some() {
            // Panel and popup layers share one priority so either surface can override base tokens
            registrations.push(CssProviderRegistration {
                layer: CssProviderLayer::Panel,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            });
        }
        if self.popup.is_some() {
            registrations.push(CssProviderRegistration {
                layer: CssProviderLayer::Popup,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            });
        }
        if self.widgets.is_some() {
            // Widget rules sit above panel shell rules so component-specific CSS can win
            registrations.push(CssProviderRegistration {
                layer: CssProviderLayer::Widgets,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
            });
        }
        if self.media.is_some() {
            // Media rules sit highest because preset layout rules are intentionally narrow
            registrations.push(CssProviderRegistration {
                layer: CssProviderLayer::Media,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3,
            });
        }
        registrations
    }

    fn provider_for_layer(&self, layer: CssProviderLayer) -> Option<&P> {
        match layer {
            CssProviderLayer::Base => Some(&self.base),
            CssProviderLayer::Panel => self.panel.as_ref(),
            CssProviderLayer::Popup => self.popup.as_ref(),
            CssProviderLayer::Widgets => self.widgets.as_ref(),
            CssProviderLayer::Media => self.media.as_ref(),
        }
    }
}

#[cfg(test)]
#[path = "tests/manager_display.rs"]
mod display_tests;
#[cfg(test)]
#[path = "tests/manager_reload.rs"]
mod reload_tests;
