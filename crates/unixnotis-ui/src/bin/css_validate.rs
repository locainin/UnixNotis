//! Isolated GTK CSS parser used by diagnostics and generated-style tests

use std::cell::{Cell, RefCell};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::CssProvider;
use serde::Serialize;

const MAX_PATH_DIAGNOSTICS: usize = 4;
const MAX_DIAGNOSTIC_MESSAGE_CHARS: usize = 512;

#[derive(Debug, Serialize)]
struct ValidatorReport {
    available: bool,
    error: Option<String>,
    truncated: bool,
    diagnostics: Vec<ValidatorDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
struct ValidatorDiagnostic {
    source: Option<PathBuf>,
    line: usize,
    column: usize,
    message: String,
}

fn main() -> ExitCode {
    // The path protocol serves installed diagnostics while stdin serves generated-style tests
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if let [flag, path] = arguments.as_slice() {
        if flag == "--json-path" {
            return run_path_protocol(Path::new(path));
        }
    }
    if arguments.is_empty() {
        return run_stdin_protocol();
    }

    eprintln!("usage: unixnotis-css-validate [--json-path PATH]");
    ExitCode::from(2)
}

fn run_path_protocol(path: &Path) -> ExitCode {
    // Initialization stays inside this helper so ordinary CLI commands do not load GTK
    let report = match gtk::init() {
        Ok(()) => {
            let (diagnostics, truncated) = parse_path(path);
            ValidatorReport {
                available: true,
                error: None,
                truncated,
                diagnostics,
            }
        }
        Err(error) => ValidatorReport {
            available: false,
            error: Some(error.to_string()),
            truncated: false,
            diagnostics: Vec::new(),
        },
    };

    // One JSON document keeps the parent-side protocol simple and deterministic
    match serde_json::to_string(&report) {
        Ok(encoded) => {
            println!("{encoded}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to encode CSS validation report: {error}");
            ExitCode::from(2)
        }
    }
}

fn run_stdin_protocol() -> ExitCode {
    // Stdin mode is intentionally small for build-time generated CSS checks
    let mut css = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut css) {
        eprintln!("failed to read css from stdin: {error}");
        return ExitCode::from(1);
    }
    if let Err(error) = gtk::init() {
        eprintln!("skipping GTK css validation: {error}");
        return ExitCode::SUCCESS;
    }

    // Parse errors are counted without retaining unbounded GTK messages
    let provider = CssProvider::new();
    let parse_errors = Rc::new(Cell::new(0usize));
    let parse_errors_for_signal = Rc::clone(&parse_errors);
    provider.connect_parsing_error(move |_, section, error| {
        let start = section.start_location();
        parse_errors_for_signal.set(parse_errors_for_signal.get() + 1);
        eprintln!(
            "gtk css parse error at line {}, col {}: {}",
            start.lines() + 1,
            start.line_chars() + 1,
            error
        );
    });
    provider.load_from_string(&css);

    // Success remains silent for easy use from build scripts
    if parse_errors.get() == 0 {
        return ExitCode::SUCCESS;
    }
    eprintln!(
        "gtk css validation found {} parse error(s)",
        parse_errors.get()
    );
    ExitCode::from(1)
}

fn parse_path(path: &Path) -> (Vec<ValidatorDiagnostic>, bool) {
    // Path mode returns structured diagnostics to the bounded parent process
    let provider = CssProvider::new();
    let diagnostics = Rc::new(RefCell::new(Vec::new()));
    let truncated = Rc::new(Cell::new(false));
    let diagnostics_for_signal = Rc::clone(&diagnostics);
    let truncated_for_signal = Rc::clone(&truncated);
    // GTK reports imported-file locations through the parsing-error signal
    provider.connect_parsing_error(move |_, section, error| {
        if diagnostics_for_signal.borrow().len() >= MAX_PATH_DIAGNOSTICS {
            truncated_for_signal.set(true);
            return;
        }
        // Locations are converted to one-based values for terminal and JSON consumers
        let start = section.start_location();
        let message = error.message();
        diagnostics_for_signal
            .borrow_mut()
            .push(ValidatorDiagnostic {
                source: section.file().and_then(|file| file.path()),
                line: start.lines() + 1,
                column: start.line_chars() + 1,
                message: unixnotis_core::util::sanitize_log_value(
                    message,
                    MAX_DIAGNOSTIC_MESSAGE_CHARS,
                ),
            });
    });
    // Loading after signal registration preserves every parser diagnostic
    provider.load_from_path(path);

    // Clone before the signal-owned reference is dropped with the provider
    let parsed = diagnostics.borrow().clone();
    (parsed, truncated.get())
}
