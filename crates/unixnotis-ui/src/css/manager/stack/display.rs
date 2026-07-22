//! Display registration order for the CSS provider stack

use gtk::gdk;

use super::super::layers::{CssProviderLayer, CssProviderRegistration};
use super::super::provider::CssProviderBackend;
use super::model::{CssManager, CssManagerInner};

impl CssManager {
    /// Register providers for the default display
    #[must_use]
    pub fn apply_to_display(&self) -> usize {
        // Returning the layer count keeps startup diagnostics observable
        self.inner.apply_to_display().len()
    }
}

impl<P> CssManagerInner<P>
where
    P: CssProviderBackend,
{
    /// Register providers for the default display and return the attempted layer plan
    pub(super) fn apply_to_display(&self) -> Vec<CssProviderRegistration> {
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

    pub(super) fn provider_registrations(&self) -> Vec<CssProviderRegistration> {
        let mut registrations = vec![
            CssProviderRegistration {
                layer: CssProviderLayer::InternalStructure,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION.saturating_sub(1),
            },
            CssProviderRegistration {
                layer: CssProviderLayer::Base,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            },
        ];
        if self.panel.is_some() {
            // Surface CSS stays above shared base tokens
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
            // Component rules override panel shell rules
            registrations.push(CssProviderRegistration {
                layer: CssProviderLayer::Widgets,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
            });
        }
        if self.media.is_some() {
            // Media geometry stays highest because it is the narrowest user layer
            registrations.push(CssProviderRegistration {
                layer: CssProviderLayer::Media,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3,
            });
        }
        if self.motion_policy.is_some() {
            // Reduced motion is an accessibility contract rather than a theme suggestion
            registrations.push(CssProviderRegistration {
                layer: CssProviderLayer::MotionPolicy,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 4,
            });
        }
        registrations
    }

    pub(super) const fn provider_for_layer(&self, layer: CssProviderLayer) -> Option<&P> {
        match layer {
            CssProviderLayer::InternalStructure => Some(&self.internal_structure),
            CssProviderLayer::Base => Some(&self.base),
            CssProviderLayer::Panel => self.panel.as_ref(),
            CssProviderLayer::Popup => self.popup.as_ref(),
            CssProviderLayer::Widgets => self.widgets.as_ref(),
            CssProviderLayer::Media => self.media.as_ref(),
            CssProviderLayer::MotionPolicy => self.motion_policy.as_ref(),
        }
    }
}
