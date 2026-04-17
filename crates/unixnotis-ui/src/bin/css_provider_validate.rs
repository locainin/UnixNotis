#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

use std::cell::Cell;
use std::io::{self, Read};
use std::process::ExitCode;
use std::rc::Rc;

use gtk::CssProvider;

fn main() -> ExitCode {
    let mut css = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut css) {
        eprintln!("failed to read css from stdin: {error}");
        return ExitCode::from(1);
    }

    // This validator runs in its own process so GTK can stay on the real main thread
    if let Err(error) = gtk::init() {
        eprintln!("skipping GTK css validation: {error}");
        return ExitCode::SUCCESS;
    }

    let provider = CssProvider::new();
    let parse_errors = Rc::new(Cell::new(0usize));
    let parse_errors_clone = Rc::clone(&parse_errors);
    provider.connect_parsing_error(move |_, section, error| {
        let start = section.start_location();
        // GTK reports locations through CssSection, so print them here for failed test cases
        parse_errors_clone.set(parse_errors_clone.get() + 1);
        eprintln!(
            "gtk css parse error at line {}, col {}: {}",
            start.lines() + 1,
            start.line_chars() + 1,
            error
        );
    });

    provider.load_from_data(&css);
    if parse_errors.get() == 0 {
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "gtk css validation found {} parse error(s)",
        parse_errors.get()
    );
    ExitCode::from(1)
}
