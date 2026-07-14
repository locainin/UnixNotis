use std::cell::Cell;

use super::helpers::{validate_with_counter, TempDirGuard};

#[test]
fn unchanged_files_reuse_cached_parse_diagnostics() {
    let root = TempDirGuard::new("unchanged-hit");
    let css_path = root.write("config/base.css", "broken-one");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    let first = validate_with_counter(
        &invocations,
        std::slice::from_ref(&css_path),
        root.path(),
        &cache_path,
    )
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

    validate_with_counter(
        &invocations,
        std::slice::from_ref(&css_path),
        root.path(),
        &cache_path,
    )
    .expect("first parse");
    let second = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 1);
    assert_eq!(second.error_count, 0);
    assert!(second.diagnostics.is_empty());
}

#[cfg(unix)]
#[test]
fn cache_symlink_is_ignored_without_touching_outside_file_or_failing_validation() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let root = TempDirGuard::new("cache-target-symlink");
    let css_path = root.write("config/base.css", "clean");
    let outside = root.write("outside.json", "keep");
    let cache_path = root.path().join("cache.json");
    symlink(&outside, &cache_path).expect("create cache symlink");
    let invocations = Cell::new(0usize);

    let report = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("cache persistence must not block validation");

    assert_eq!(report.error_count, 0);
    assert_eq!(invocations.get(), 1);
    assert_eq!(
        fs::read_to_string(outside).expect("read outside file"),
        "keep"
    );
}
