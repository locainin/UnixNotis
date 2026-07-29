//! CSS stack state and surface-specific construction

use gtk::CssProvider;
use unixnotis_core::{ThemeConfig, ThemeContractState, ThemeMode, ThemePaths};

use super::super::provider::CssProviderBackend;

/// Identifies which UI surface is loading CSS
#[derive(Clone, Copy, Debug)]
pub enum CssKind {
    Panel,
    Popup,
}

/// CSS provider stack for `UnixNotis` windows
#[derive(Clone)]
pub struct CssManager {
    // The generic inner stack keeps provider order testable without a display
    pub(super) inner: CssManagerInner<CssProvider>,
}

impl CssManager {
    #[must_use]
    pub fn new_panel(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        // Panel stacks own panel, widget, and media layers but never popup CSS
        Self {
            inner: CssManagerInner::new_panel(theme_paths, theme_config),
        }
    }

    #[must_use]
    pub fn new_popup(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        // Popup stacks stay isolated so panel-only selectors cannot affect popup sizing
        Self {
            inner: CssManagerInner::new_popup(theme_paths, theme_config),
        }
    }

    /// Return the path bundle used by the next reload
    #[must_use]
    pub const fn theme_paths(&self) -> &ThemePaths {
        &self.inner.theme_paths
    }

    /// Return the source contract selected for the next reload
    #[must_use]
    pub fn theme_contract(&self) -> ThemeContractState {
        self.inner.theme_contract()
    }
}

#[derive(Clone)]
pub(super) struct CssManagerInner<P>
where
    P: CssProviderBackend,
{
    // Paths and alpha settings are swapped together during accepted config reloads
    pub(super) theme_paths: ThemePaths,
    pub(super) theme_config: ThemeConfig,
    // Structural CSS always loads below every editable user layer
    pub(super) internal_structure: P,
    pub(super) base: P,
    pub(super) panel: Option<P>,
    pub(super) widgets: Option<P>,
    pub(super) media: Option<P>,
    // Runtime accessibility policy must override every editable panel theme layer
    pub(super) motion_policy: Option<P>,
    pub(super) popup: Option<P>,
}

impl CssManagerInner<CssProvider> {
    fn new_panel(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        // Provider slots are stable across reloads so GTK registrations do not accumulate
        Self {
            theme_paths,
            theme_config,
            internal_structure: CssProvider::new(),
            base: CssProvider::new(),
            panel: Some(CssProvider::new()),
            widgets: Some(CssProvider::new()),
            media: Some(CssProvider::new()),
            motion_policy: Some(CssProvider::new()),
            popup: None,
        }
    }

    fn new_popup(theme_paths: ThemePaths, theme_config: ThemeConfig) -> Self {
        // Absent panel slots prevent accidental loading of unrelated files
        Self {
            theme_paths,
            theme_config,
            internal_structure: CssProvider::new(),
            base: CssProvider::new(),
            panel: None,
            widgets: None,
            media: None,
            motion_policy: None,
            popup: Some(CssProvider::new()),
        }
    }
}

impl<P> CssManagerInner<P>
where
    P: CssProviderBackend,
{
    pub(super) fn theme_contract(&self) -> ThemeContractState {
        match self.theme_config.mode {
            ThemeMode::Stock => ThemeContractState::EmbeddedStock,
            ThemeMode::Custom => self.theme_paths.inspect_theme_contract(),
        }
    }
}
