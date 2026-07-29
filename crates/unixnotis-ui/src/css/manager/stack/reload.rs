//! CSS layer loading and explicit reload reporting

use unixnotis_core::{
    ThemeConfig, ThemePaths, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS, INTERNAL_STRUCTURE_CSS, MOTION_POLICY_CSS,
};

use super::super::super::loader::{
    load_embedded_provider_with_overrides, load_provider_with_overrides, CssFileLoadResult,
    CssFileLoadSource,
};
use super::super::super::overrides::{
    build_base_overrides, build_panel_overrides, build_popup_overrides, build_widgets_overrides,
};
use super::super::layers::CssProviderLayer;
use super::super::provider::CssProviderBackend;
use super::super::report::{CssLayerReload, CssLayerSource, CssReloadReport};
use super::model::{CssManager, CssManagerInner};

impl CssManager {
    /// Reload CSS from disk or fall back to embedded defaults
    #[must_use]
    pub fn reload(&self, fallback: &str) -> CssReloadReport {
        // The public wrapper keeps backend details out of callers
        self.inner.reload(fallback)
    }

    pub fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        // Paths and generated tokens change together for the next reload
        self.inner.update_theme(theme_paths, theme_config);
    }
}

impl<P> CssManagerInner<P>
where
    P: CssProviderBackend,
{
    pub(super) fn reload(&self, fallback: &str) -> CssReloadReport {
        let mut loaded = Vec::new();
        let custom_theme_allowed = self.theme_contract().custom_theme_allowed();
        // Structural fallbacks always load below every user-controlled layer
        self.internal_structure
            .load_css_data(INTERNAL_STRUCTURE_CSS);

        // Base variables load before every surface-specific provider
        let base_overrides = build_base_overrides(&self.theme_config);
        let result = load_provider(
            custom_theme_allowed,
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

        // Optional providers distinguish panel and popup process layouts
        if let Some(panel) = self.panel.as_ref() {
            let panel_overrides = build_panel_overrides(&self.theme_config);
            let result = load_provider(
                custom_theme_allowed,
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

        // Widget overrides remain isolated from panel structural rules
        if let Some(widgets) = self.widgets.as_ref() {
            let widgets_overrides = build_widgets_overrides(&self.theme_config);
            let result = load_provider(
                custom_theme_allowed,
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

        // Media has no generated override layer and remains fully theme controlled
        if let Some(media) = self.media.as_ref() {
            let result = load_provider(
                custom_theme_allowed,
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

        // Popup geometry tokens apply only when the popup provider exists
        if let Some(popup) = self.popup.as_ref() {
            let popup_overrides = build_popup_overrides(&self.theme_config);
            let result = load_provider(
                custom_theme_allowed,
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

        if let Some(motion_policy) = self.motion_policy.as_ref() {
            // This fixed policy is intentionally loaded after every editable panel layer
            motion_policy.load_css_data(MOTION_POLICY_CSS);
        }

        // Callers receive every layer outcome instead of a lossy success count
        CssReloadReport { layers: loaded }
    }

    pub(super) fn update_theme(&mut self, theme_paths: ThemePaths, theme_config: ThemeConfig) {
        // The next reload must observe paths and generated tokens from one config
        self.theme_paths = theme_paths;
        self.theme_config = theme_config;
    }
}

fn load_provider(
    custom_theme_allowed: bool,
    load_css_data: impl Fn(&str),
    path: &std::path::Path,
    fallback: &str,
    overrides: &str,
    inject_base_tokens: bool,
) -> CssFileLoadResult {
    if custom_theme_allowed {
        load_provider_with_overrides(load_css_data, path, fallback, overrides, inject_base_tokens)
    } else {
        load_embedded_provider_with_overrides(
            load_css_data,
            path,
            fallback,
            overrides,
            inject_base_tokens,
        )
    }
}

fn layer_reload(
    layer: CssProviderLayer,
    path: std::path::PathBuf,
    result: CssFileLoadResult,
) -> CssLayerReload {
    // Loader sources map into the stable public report vocabulary
    let source = match result.source {
        CssFileLoadSource::EmbeddedStock => CssLayerSource::EmbeddedStock,
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
