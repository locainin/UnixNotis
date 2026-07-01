use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

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
