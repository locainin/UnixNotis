use std::cell::Cell;
use std::fs;

use super::super::{parse_diagnostic_for_test, validate_css_parse_files_with};
use super::helpers::{pause_for_metadata_tick, validate_with_counter, TempDirGuard};

#[test]
fn imported_css_changes_miss_the_cache() {
    let root = TempDirGuard::new("import-change");
    let css_path = root.write("config/base.css", "@import url(\"imported.css\");");
    let imported_path = root.write("config/imported.css", "broken-one");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    let first = validate_css_parse_files_with(
        std::slice::from_ref(&css_path),
        root.path(),
        "$TMP/unixnotis",
        &cache_path,
        |_work_item| {
            invocations.set(invocations.get() + 1);
            let contents = fs::read_to_string(&imported_path)?;
            Ok(parse_diagnostic_for_test(contents))
        },
    )
    .expect("first parse");
    assert!(first.diagnostics[0].message.contains("broken-one"));

    pause_for_metadata_tick();
    fs::write(&imported_path, "broken-two").expect("rewrite imported css");

    let second = validate_css_parse_files_with(
        &[css_path],
        root.path(),
        "$TMP/unixnotis",
        &cache_path,
        |_work_item| {
            invocations.set(invocations.get() + 1);
            let contents = fs::read_to_string(&imported_path)?;
            Ok(parse_diagnostic_for_test(contents))
        },
    )
    .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert!(second.diagnostics[0].message.contains("broken-two"));
}

#[test]
fn missing_import_that_appears_misses_the_cache() {
    let root = TempDirGuard::new("import-appears");
    let css_path = root.write("config/base.css", "@import \"imported.css\";");
    let imported_path = root.path().join("config/imported.css");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    let first = validate_css_parse_files_with(
        std::slice::from_ref(&css_path),
        root.path(),
        "$TMP/unixnotis",
        &cache_path,
        |_work_item| {
            invocations.set(invocations.get() + 1);
            if imported_path.exists() {
                Ok(Vec::new())
            } else {
                Ok(parse_diagnostic_for_test("missing import"))
            }
        },
    )
    .expect("first parse");
    assert_eq!(first.error_count, 1);

    pause_for_metadata_tick();
    fs::write(&imported_path, ".ok { color: red; }").expect("create imported css");

    let second = validate_css_parse_files_with(
        &[css_path],
        root.path(),
        "$TMP/unixnotis",
        &cache_path,
        |_work_item| {
            invocations.set(invocations.get() + 1);
            if imported_path.exists() {
                Ok(Vec::new())
            } else {
                Ok(parse_diagnostic_for_test("missing import"))
            }
        },
    )
    .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert_eq!(second.error_count, 0);
    assert!(second.diagnostics.is_empty());
}

#[test]
fn top_level_files_still_reuse_cache_when_imports_do_not_change() {
    let root = TempDirGuard::new("import-stable");
    let css_path = root.write("config/base.css", "@import url(\"imported.css\");");
    root.write("config/imported.css", ".ok { color: red; }");
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
    assert_eq!(second.error_count, 1);
}
