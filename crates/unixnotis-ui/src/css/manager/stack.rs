//! CSS provider management and reload orchestration for `UnixNotis` UIs

use gtk::gdk;
use gtk::CssProvider;
use unixnotis_core::{
    ThemeConfig, ThemePaths, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

use super::super::loader::{load_provider_with_overrides, CssFileLoadResult, CssFileLoadSource};
use super::super::overrides::{
    build_base_overrides, build_panel_overrides, build_popup_overrides, build_widgets_overrides,
};

use super::layers::{CssProviderLayer, CssProviderRegistration};
use super::provider::CssProviderBackend;
use super::report::{CssLayerReload, CssLayerSource, CssReloadReport};

/// Identifies which UI surface is loading CSS
#[derive(Clone, Copy, Debug)]
pub enum CssKind {
    Panel,
    Popup,
}

/// CSS provider stack for `UnixNotis` windows
#[derive(Clone)]
pub struct CssManager {
    inner: CssManagerInner<CssProvider>,
}

impl CssManager {
    #[must_use]
    pub fn new_panel(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            inner: CssManagerInner::new_panel(theme_paths, theme_config),
        }
    }

    #[must_use]
    pub fn new_popup(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        Self {
            inner: CssManagerInner::new_popup(theme_paths, theme_config),
        }
    }

    /// Register providers for the default display
    #[must_use]
    pub fn apply_to_display(&self) -> usize {
        // Returning the layer count makes startup diagnostics and tests observable
        self.inner.apply_to_display().len()
    }

    /// Reload CSS from disk or fall back to embedded defaults
    #[must_use]
    pub fn reload(&self, fallback: &str) -> CssReloadReport {
        self.inner.reload(fallback)
    }

    pub fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        self.inner.update_theme(theme_paths, theme_config);
    }

    /// Return the path bundle used by the next reload
    #[must_use]
    pub const fn theme_paths(&self) -> &ThemePaths {
        &self.inner.theme_paths
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
    /// Register providers for the default display and return the attempted layer plan
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

    /// Reload CSS from disk or fall back to embedded defaults
    fn reload(&self, fallback: &str) -> CssReloadReport {
        let mut loaded = Vec::new();
        // Base CSS gets the token injection to preserve alpha calculations
        let base_overrides = build_base_overrides(&self.theme_config);
        let result = load_provider_with_overrides(
            |data| self.base.load_css_data(data),
            &self.theme_paths.base_css,
            fallback,
            &base_overrides,
            true,
        );
        loaded.push(layer_reload(
            CssProviderLayer::Base,
            self.theme_paths.base_css.clone(),
            result,
        ));

        if let Some(panel) = self.panel.as_ref() {
            // Panel overrides are generated from typed theme values before custom CSS wins
            let panel_overrides = build_panel_overrides(&self.theme_config);
            let result = load_provider_with_overrides(
                |data| panel.load_css_data(data),
                &self.theme_paths.panel_css,
                DEFAULT_PANEL_CSS,
                &panel_overrides,
                false,
            );
            loaded.push(layer_reload(
                CssProviderLayer::Panel,
                self.theme_paths.panel_css.clone(),
                result,
            ));
        }

        if let Some(widgets) = self.widgets.as_ref() {
            // Widget tokens stay isolated from shell geometry and popup styling
            let widgets_overrides = build_widgets_overrides(&self.theme_config);
            let result = load_provider_with_overrides(
                |data| widgets.load_css_data(data),
                &self.theme_paths.widgets_css,
                DEFAULT_WIDGETS_CSS,
                &widgets_overrides,
                false,
            );
            loaded.push(layer_reload(
                CssProviderLayer::Widgets,
                self.theme_paths.widgets_css.clone(),
                result,
            ));
        }

        if let Some(media) = self.media.as_ref() {
            // Media css is intentionally isolated so ricing one widget does not pollute widgets.css
            let result = load_provider_with_overrides(
                |data| media.load_css_data(data),
                &self.theme_paths.media_css,
                DEFAULT_MEDIA_CSS,
                "",
                false,
            );
            loaded.push(layer_reload(
                CssProviderLayer::Media,
                self.theme_paths.media_css.clone(),
                result,
            ));
        }

        if let Some(popup) = self.popup.as_ref() {
            // Popup processes register only base and popup providers
            let popup_overrides = build_popup_overrides(&self.theme_config);
            let result = load_provider_with_overrides(
                |data| popup.load_css_data(data),
                &self.theme_paths.popup_css,
                DEFAULT_POPUP_CSS,
                &popup_overrides,
                false,
            );
            loaded.push(layer_reload(
                CssProviderLayer::Popup,
                self.theme_paths.popup_css.clone(),
                result,
            ));
        }

        CssReloadReport { layers: loaded }
    }

    fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        // Store inputs so the next reload picks up new paths and override settings
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

    const fn provider_for_layer(&self, layer: CssProviderLayer) -> Option<&P> {
        match layer {
            CssProviderLayer::Base => Some(&self.base),
            CssProviderLayer::Panel => self.panel.as_ref(),
            CssProviderLayer::Popup => self.popup.as_ref(),
            CssProviderLayer::Widgets => self.widgets.as_ref(),
            CssProviderLayer::Media => self.media.as_ref(),
        }
    }
}

fn layer_reload(
    layer: CssProviderLayer,
    path: std::path::PathBuf,
    result: CssFileLoadResult,
) -> CssLayerReload {
    // Internal loader outcomes map one-to-one onto the public reload report
    let source = match result.source {
        CssFileLoadSource::Custom => CssLayerSource::Custom,
        CssFileLoadSource::EmptyFallback => CssLayerSource::EmptyFallback,
        CssFileLoadSource::ReadFailureFallback => CssLayerSource::ReadFailureFallback,
    };
    CssLayerReload {
        layer,
        path,
        source,
        error: result.error,
    }
}

#[cfg(test)]
#[path = "tests/stack_display.rs"]
mod display_tests;
#[cfg(test)]
#[path = "tests/stack_reload.rs"]
mod reload_tests;
