use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use unixnotis_ui::css::{CssLayerReload, CssLayerSource, CssProviderLayer, CssReloadReport};

struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_poisoned| io::Error::other("captured log lock poisoned"))?
            .write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

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

#[test]
fn css_reload_logs_only_the_filename_and_stable_failure_state() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer_output = Arc::clone(&output);
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || CapturedWriter(Arc::clone(&writer_output)))
        .finish();
    let report = CssReloadReport {
        layers: vec![CssLayerReload {
            layer: CssProviderLayer::Popup,
            path: PathBuf::from("/private-popup-root/theme/popup.css"),
            source: CssLayerSource::ReadFailureFallback,
            error: Some("private-css-error-sentinel".to_string()),
        }],
    };

    tracing::subscriber::with_default(subscriber, || {
        super::log_reload_failures(&report, "test reload");
    });

    let rendered = String::from_utf8(output.lock().expect("lock captured CSS output").clone())
        .expect("CSS output should be UTF-8");
    assert!(rendered.contains("popup.css"));
    assert!(rendered.contains("read_error=true"));
    assert!(!rendered.contains("/private-popup-root"));
    assert!(!rendered.contains("private-css-error-sentinel"));
}
