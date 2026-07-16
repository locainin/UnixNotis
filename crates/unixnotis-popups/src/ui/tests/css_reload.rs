use std::path::PathBuf;

use unixnotis_ui::css::{CssLayerReload, CssLayerSource, CssProviderLayer, CssReloadReport};

#[test]
fn read_failure_iterator_excludes_intentional_empty_fallbacks() {
    let report = CssReloadReport {
        layers: vec![
            CssLayerReload {
                layer: CssProviderLayer::Base,
                path: PathBuf::from("base.css"),
                source: CssLayerSource::EmptyFallback,
                error: None,
            },
            CssLayerReload {
                layer: CssProviderLayer::Popup,
                path: PathBuf::from("popup.css"),
                source: CssLayerSource::ReadFailureFallback,
                error: Some("missing".to_string()),
            },
        ],
    };

    let failures = report.read_failures().collect::<Vec<_>>();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].layer, CssProviderLayer::Popup);
}
