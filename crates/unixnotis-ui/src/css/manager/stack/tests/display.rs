use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::gdk;
use unixnotis_core::{ThemeConfig, ThemePaths};

use super::super::model::{CssManager, CssManagerInner};
use crate::css::manager::layers::{CssProviderLayer, CssProviderRegistration};
use crate::css::manager::provider::CssProviderBackend;

#[derive(Clone)]
struct RecordingProvider {
    label: &'static str,
    calls: Rc<RefCell<Vec<&'static str>>>,
}

impl RecordingProvider {
    fn new(label: &'static str, calls: Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self { label, calls }
    }
}

impl CssProviderBackend for RecordingProvider {
    fn load_css_data(&self, _data: &str) {}

    fn add_to_display(&self, _display: &gdk::Display, _priority: u32) {
        // Display calls are recorded for completeness when a test environment has a display
        self.calls.borrow_mut().push(self.label);
    }
}

fn theme_paths(root: &str) -> ThemePaths {
    let base = PathBuf::from(root);
    ThemePaths {
        base_dir: base.clone(),
        base_css: base.join("base.css"),
        popup_css: base.join("popup.css"),
        panel_css: base.join("panel.css"),
        widgets_css: base.join("widgets.css"),
        media_css: base.join("media.css"),
    }
}

#[test]
fn panel_manager_registers_base_panel_widgets_and_media_priorities() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let manager = CssManagerInner {
        theme_paths: theme_paths("/tmp/unixnotis-panel-css"),
        theme_config: ThemeConfig::default(),
        internal_structure: RecordingProvider::new("internal", Rc::clone(&calls)),
        base: RecordingProvider::new("base", Rc::clone(&calls)),
        panel: Some(RecordingProvider::new("panel", Rc::clone(&calls))),
        widgets: Some(RecordingProvider::new("widgets", Rc::clone(&calls))),
        media: Some(RecordingProvider::new("media", Rc::clone(&calls))),
        popup: None,
    };

    let registrations = manager.apply_to_display();

    // The returned plan is stable even when no GTK display exists in headless tests
    assert_eq!(
        registrations,
        vec![
            CssProviderRegistration {
                layer: CssProviderLayer::InternalStructure,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1,
            },
            CssProviderRegistration {
                layer: CssProviderLayer::Base,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            },
            CssProviderRegistration {
                layer: CssProviderLayer::Panel,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            },
            CssProviderRegistration {
                layer: CssProviderLayer::Widgets,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
            },
            CssProviderRegistration {
                layer: CssProviderLayer::Media,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3,
            },
        ]
    );
}

#[test]
fn popup_manager_registers_base_and_popup_at_popup_priority() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let manager = CssManagerInner {
        theme_paths: theme_paths("/tmp/unixnotis-popup-css"),
        theme_config: ThemeConfig::default(),
        internal_structure: RecordingProvider::new("internal", Rc::clone(&calls)),
        base: RecordingProvider::new("base", Rc::clone(&calls)),
        panel: None,
        widgets: None,
        media: None,
        popup: Some(RecordingProvider::new("popup", Rc::clone(&calls))),
    };

    let registrations = manager.apply_to_display();

    assert_eq!(
        registrations,
        vec![
            CssProviderRegistration {
                layer: CssProviderLayer::InternalStructure,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1,
            },
            CssProviderRegistration {
                layer: CssProviderLayer::Base,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            },
            CssProviderRegistration {
                layer: CssProviderLayer::Popup,
                priority: gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            },
        ]
    );
}

#[gtk::test]
fn public_panel_manager_reports_every_registered_provider() {
    let manager = CssManager::new_panel(
        theme_paths("/tmp/unixnotis-public-panel-css"),
        ThemeConfig::default(),
    );

    assert_eq!(manager.apply_to_display(), 5);
}

#[test]
fn provider_lookup_returns_only_layers_owned_by_the_manager() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let manager = CssManagerInner {
        theme_paths: theme_paths("/tmp/unixnotis-provider-lookup-css"),
        theme_config: ThemeConfig::default(),
        internal_structure: RecordingProvider::new("internal", Rc::clone(&calls)),
        base: RecordingProvider::new("base", Rc::clone(&calls)),
        panel: Some(RecordingProvider::new("panel", Rc::clone(&calls))),
        widgets: None,
        media: None,
        popup: None,
    };

    assert_eq!(
        manager
            .provider_for_layer(CssProviderLayer::InternalStructure)
            .map(|provider| provider.label),
        Some("internal")
    );
    assert_eq!(
        manager
            .provider_for_layer(CssProviderLayer::Base)
            .map(|provider| provider.label),
        Some("base")
    );
    assert_eq!(
        manager
            .provider_for_layer(CssProviderLayer::Panel)
            .map(|provider| provider.label),
        Some("panel")
    );
    assert!(manager
        .provider_for_layer(CssProviderLayer::Popup)
        .is_none());
}
