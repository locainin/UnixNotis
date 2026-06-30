//! CSS provider layer ordering types

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CssProviderLayer {
    Base,
    Panel,
    Popup,
    Widgets,
    Media,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CssProviderRegistration {
    pub(super) layer: CssProviderLayer,
    pub(super) priority: u32,
}
