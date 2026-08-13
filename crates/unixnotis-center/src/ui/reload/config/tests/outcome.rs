use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use unixnotis_core::ConfigError;

use super::super::outcome::{log_reload_rejection, ReloadFailure};

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
fn reload_failure_kinds_remain_stable_for_structured_logs() {
    assert_eq!(
        ReloadFailure::Config(ConfigError::MissingHome).kind(),
        "config"
    );
    assert_eq!(
        ReloadFailure::ThemeBase("missing".to_string()).kind(),
        "theme-base"
    );
    assert_eq!(
        ReloadFailure::ThemePaths("invalid".to_string()).kind(),
        "theme-paths"
    );
}

#[test]
fn rejected_config_logs_never_include_private_parser_text() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer_output = Arc::clone(&output);
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || CapturedWriter(Arc::clone(&writer_output)))
        .finish();
    let failure = ReloadFailure::Config(ConfigError::ParseFailed(
        "private-center-parser-sentinel".to_string(),
    ));

    tracing::subscriber::with_default(subscriber, || log_reload_rejection(&failure));

    let rendered = String::from_utf8(output.lock().expect("lock captured center output").clone())
        .expect("center output should be UTF-8");
    assert!(rendered.contains("kind=\"config\""));
    assert!(rendered.contains("fingerprint="));
    assert!(!rendered.contains("private-center-parser-sentinel"));
}
