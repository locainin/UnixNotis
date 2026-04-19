use std::cell::Cell;

use super::helpers::{validate_with_counter, TempDirGuard};

#[test]
fn unchanged_files_reuse_cached_parse_diagnostics() {
    let root = TempDirGuard::new("unchanged-hit");
    let css_path = root.write("config/base.css", "broken-one");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    let first = validate_with_counter(&invocations, &[css_path.clone()], root.path(), &cache_path)
        .expect("first parse");
    let second = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 1);
    assert_eq!(first.error_count, 1);
    assert_eq!(second.error_count, 1);
    assert_eq!(first.diagnostics, second.diagnostics);
}

#[test]
fn unchanged_clean_files_do_not_reparse() {
    let root = TempDirGuard::new("unchanged-clean");
    let css_path = root.write("config/base.css", "clean");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(&invocations, &[css_path.clone()], root.path(), &cache_path)
        .expect("first parse");
    let second = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 1);
    assert_eq!(second.error_count, 0);
    assert!(second.diagnostics.is_empty());
}
