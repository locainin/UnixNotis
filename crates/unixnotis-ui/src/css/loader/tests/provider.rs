//! CSS provider loading tests

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use unixnotis_core::MAX_CSS_FILE_BYTES;

use super::*;

fn unique_css_test_dir(label: &str) -> PathBuf {
    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    // A per-test directory avoids cross-test races while keeping dependencies small
    let unique = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "unixnotis-ui-css-loader-{pid}-{label}-{unique}",
        pid = std::process::id(),
    ));
    fs::create_dir_all(&path).expect("create css test directory");
    path
}

#[test]
fn load_provider_with_overrides_loads_merged_and_rebased_css_into_sink() {
    let root = unique_css_test_dir("load-provider");
    let css_dir = root.join("themes");
    let css_path = css_dir.join("widgets.css");
    let loaded = RefCell::new(Vec::<String>::new());

    fs::create_dir_all(&css_dir).expect("create css fixture directory");
    fs::write(
        &css_path,
        ".card { background-image: url(icons/card.png); color: green; }",
    )
    .expect("write css fixture");

    load_provider_with_overrides(
        |data| {
            // Tests inspect the exact bytes sent to GTK without needing a display server
            loaded.borrow_mut().push(data.to_string());
        },
        &css_path,
        ".card { color: red; }",
        ".card { color: blue; }",
        false,
    );

    let loaded = loaded.borrow();
    assert_eq!(loaded.len(), 1);
    // Edited user CSS keeps overrides first, then rebases asset refs before GTK sees the data
    assert!(loaded[0].starts_with(".card { color: blue; }\n.card"));
    assert!(loaded[0].contains("file://"));
    assert!(loaded[0].contains("/themes/icons/card.png"));

    fs::remove_dir_all(root).expect("remove css test directory");
}

#[test]
fn empty_css_uses_the_embedded_fallback() {
    let root = unique_css_test_dir("empty");
    let path = root.join("popup.css");
    fs::write(&path, "\n  \t").expect("write empty stylesheet");
    let loaded = RefCell::new(Vec::new());

    let result = load_provider_with_overrides(
        |data| loaded.borrow_mut().push(data.to_string()),
        &path,
        ".fallback { color: red; }",
        "",
        false,
    );

    assert_eq!(result.source, CssFileLoadSource::EmptyFallback);
    assert_eq!(loaded.borrow().as_slice(), [".fallback { color: red; }"]);
    fs::remove_dir_all(root).expect("remove css test directory");
}

#[test]
fn unsafe_or_invalid_css_files_use_the_embedded_fallback() {
    let root = unique_css_test_dir("unsafe");
    let fallback = ".fallback { color: red; }";
    let cases = [
        "missing",
        "invalid-utf8",
        "directory",
        "symlink",
        "oversized",
    ];

    for case in cases {
        let path = root.join(case);
        match case {
            "missing" => {}
            "invalid-utf8" => fs::write(&path, [0xff, 0xfe]).expect("write invalid CSS"),
            "directory" => fs::create_dir(&path).expect("create CSS directory"),
            "symlink" => {
                let target = root.join("symlink-target.css");
                fs::write(&target, ".target { color: blue; }").expect("write symlink target");
                std::os::unix::fs::symlink(&target, &path).expect("create CSS symlink");
            }
            "oversized" => {
                let file = fs::File::create(&path).expect("create oversized CSS");
                file.set_len(MAX_CSS_FILE_BYTES + 1)
                    .expect("make CSS file oversized");
            }
            _ => unreachable!("all cases are covered above"),
        }

        let loaded = RefCell::new(Vec::new());
        let result = load_provider_with_overrides(
            |data| loaded.borrow_mut().push(data.to_string()),
            &path,
            fallback,
            "",
            false,
        );

        assert_eq!(
            result.source,
            CssFileLoadSource::ReadFailureFallback,
            "{case}"
        );
        assert_eq!(loaded.borrow().as_slice(), [fallback], "{case}");
    }

    fs::remove_dir_all(root).expect("remove css test directory");
}

#[test]
fn configured_css_reload_replaces_the_previous_contents() {
    let root = unique_css_test_dir("reload");
    let path = root.join("popup.css");
    let loaded = RefCell::new(Vec::new());

    fs::write(&path, ".popup { color: red; }").expect("write first stylesheet");
    load_provider_with_overrides(
        |data| loaded.borrow_mut().push(data.to_string()),
        &path,
        ".fallback {}",
        "",
        false,
    );
    fs::write(&path, ".popup { color: green; }").expect("write second stylesheet");
    load_provider_with_overrides(
        |data| loaded.borrow_mut().push(data.to_string()),
        &path,
        ".fallback {}",
        "",
        false,
    );

    let loaded = loaded.borrow();
    assert!(loaded[0].contains("red"));
    assert!(loaded[1].contains("green"));
    fs::remove_dir_all(root).expect("remove css test directory");
}
