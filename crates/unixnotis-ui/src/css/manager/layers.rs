//! CSS provider layer ordering types

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssProviderLayer {
    /// Shared token and common component layer
    Base,
    /// Control-center shell layer
    Panel,
    /// Popup surface layer
    Popup,
    /// Quick-control and custom-widget layer
    Widgets,
    /// MPRIS media card layer
    Media,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CssProviderRegistration {
    pub(super) layer: CssProviderLayer,
    pub(super) priority: u32,
}
