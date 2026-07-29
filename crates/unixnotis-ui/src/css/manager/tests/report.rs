use std::path::PathBuf;

use super::*;

#[test]
fn read_failures_excludes_custom_and_intentional_empty_fallbacks() {
    let report = CssReloadReport {
        layers: vec![
            CssLayerReload {
                layer: CssProviderLayer::Popup,
                path: PathBuf::from("popup.css"),
                source: CssLayerSource::EmbeddedStock,
                error: None,
            },
            CssLayerReload {
                layer: CssProviderLayer::Base,
                path: PathBuf::from("base.css"),
                source: CssLayerSource::Custom,
                error: None,
            },
            CssLayerReload {
                layer: CssProviderLayer::Panel,
                path: PathBuf::from("panel.css"),
                source: CssLayerSource::EmptyFallback,
                error: None,
            },
            CssLayerReload {
                layer: CssProviderLayer::Media,
                path: PathBuf::from("media.css"),
                source: CssLayerSource::ReadFailureFallback,
                error: Some("missing".to_string()),
            },
        ],
    };

    let failures = report.read_failures().collect::<Vec<_>>();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].layer, CssProviderLayer::Media);
}
