use std::cell::Cell;
use std::rc::Rc;

use super::{CssProvider, CssProviderBackend};

#[gtk::test]
fn gtk_provider_backend_loads_css_and_reports_invalid_input() {
    let provider = CssProvider::new();
    let parse_errors = Rc::new(Cell::new(0));
    let observed_errors = parse_errors.clone();
    provider.connect_parsing_error(move |_, _, _| {
        observed_errors.set(observed_errors.get() + 1);
    });

    provider.load_css_data(".broken { color: ;");

    assert!(parse_errors.get() > 0);
}
