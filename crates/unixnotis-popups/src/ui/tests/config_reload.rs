use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use super::*;

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
fn rejected_config_logs_never_include_private_parser_text() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer_output = Arc::clone(&output);
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || CapturedWriter(Arc::clone(&writer_output)))
        .finish();
    let error = ConfigError::ParseFailed("private-popup-parser-sentinel".to_string());

    tracing::subscriber::with_default(subscriber, || log_config_rejection(&error));

    let rendered = String::from_utf8(output.lock().expect("lock captured popup output").clone())
        .expect("popup output should be UTF-8");
    assert!(rendered.contains("kind=\"parse\""));
    assert!(rendered.contains(error.shareable_summary()));
    assert!(!rendered.contains("private-popup-parser-sentinel"));
}

#[test]
fn oversized_config_uses_a_stable_rejection_kind() {
    let error = ConfigError::TooLarge {
        size: 2_000_000,
        max: 1_048_576,
    };

    assert_eq!(config_error_kind(&error), "too-large");
}

#[test]
fn theme_resolution_failure_logs_only_the_stable_stage() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer_output = Arc::clone(&output);
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || CapturedWriter(Arc::clone(&writer_output)))
        .finish();

    tracing::subscriber::with_default(subscriber, || log_theme_resolution_failure("config-reload"));

    let rendered = String::from_utf8(output.lock().expect("lock captured popup output").clone())
        .expect("popup output should be UTF-8");
    assert!(rendered.contains("stage=config-reload"));
    assert!(rendered.contains("failed to resolve popup theme inputs"));
}
