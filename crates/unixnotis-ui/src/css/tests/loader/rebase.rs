use std::path::Path;

use super::*;

#[test]
fn rebase_relative_css_asset_urls_rewrites_quoted_relative_path() {
    let css = ".card { background-image: url(\"../assets/example-image.png\"); }";
    let css_path = Path::new("/tmp/unixnotis/themes/widgets.css");

    let rebased = rebase_relative_css_asset_urls(css, css_path);

    // Relative assets are anchored to the stylesheet directory before GTK loads merged CSS bytes
    assert!(rebased.contains("file:///tmp/unixnotis/assets/example-image.png"));
    assert!(rebased.contains("url(\"file:///tmp/unixnotis/assets/example-image.png\")"));
}

#[test]
fn rebase_relative_css_asset_urls_rewrites_single_quoted_and_unquoted_paths() {
    let css = ".a { background: url('../a one.png'); }\n.b { mask-image: URL(icons/b.svg); }";
    let css_path = Path::new("/tmp/unixnotis/themes/widgets.css");

    let rebased = rebase_relative_css_asset_urls(css, css_path);

    // Both common authoring styles need the same file URI treatment
    assert!(rebased.contains("file:///tmp/unixnotis/a%20one.png"));
    assert!(rebased.contains("file:///tmp/unixnotis/themes/icons/b.svg"));
}

#[test]
fn rebase_relative_css_asset_urls_preserves_unicode_path() {
    let css = ".card { background-image: url(\"icons/café.png\"); }";
    let css_path = Path::new("/tmp/unixnotis/themes/widgets.css");

    let rebased = rebase_relative_css_asset_urls(css, css_path);

    // Non-ASCII asset names should survive parsing and then be percent-encoded in the URI
    assert!(rebased.contains("file:///tmp/unixnotis/themes/icons/caf%C3%A9.png"));
}

#[test]
fn rebase_relative_css_asset_urls_preserves_portable_percent_encoding_once() {
    let css = concat!(
        ".a { background: url(../assets/icon%20one.png); }\n",
        ".b { background: url(\"../assets/icon%23one.png\"); }\n",
        ".c { background: url('../assets/icon%25one.png'); }\n",
        ".d { background: url(../assets/icon%29one.png); }\n",
        ".e { background: url(\"../assets/icon%22one.png\"); }",
    );
    let css_path = Path::new("/tmp/unixnotis/themes/widgets.css");

    let rebased = rebase_relative_css_asset_urls(css, css_path);

    for encoded_name in [
        "icon%20one.png",
        "icon%23one.png",
        "icon%25one.png",
        "icon%29one.png",
        "icon%22one.png",
    ] {
        assert!(
            rebased.contains(&format!("file:///tmp/unixnotis/assets/{encoded_name}")),
            "missing encoded target {encoded_name}: {rebased}"
        );
    }
    assert!(!rebased.contains("%2520"));
    assert!(!rebased.contains("%2523"));
    assert!(!rebased.contains("%2525"));
    assert!(!rebased.contains("%2529"));
    assert!(!rebased.contains("%2522"));
}

#[test]
fn rebase_relative_css_asset_urls_keeps_other_absolute_uri_schemes_external() {
    let css = concat!(
        ".a { background-image: url(ftp://example.invalid/a.png); }\n",
        ".b { background-image: url(\"ipfs://example/a.png\"); }\n",
        ".c { background-image: url(custom:asset-name); }",
    );
    let css_path = Path::new("/tmp/unixnotis/widgets.css");

    let rebased = rebase_relative_css_asset_urls(css, css_path);

    assert_eq!(rebased, css);
}

#[test]
fn rebase_relative_css_asset_urls_keeps_absolute_remote_data_and_file_urls() {
    let css = concat!(
        ".a { background-image: url(\"file:///tmp/outside.png\"); }\n",
        ".b { background-image: url(\"https://example.com/test.png\"); }\n",
        ".c { background-image: url(\"data:image/png;base64,abcd\"); }\n",
        ".d { background-image: url(\"/usr/share/pixmaps/icon.png\"); }",
    );
    let css_path = Path::new("/tmp/unixnotis/widgets.css");

    let rebased = rebase_relative_css_asset_urls(css, css_path);

    // These targets are already explicit and must not be rewritten as config-relative files
    assert!(rebased.contains("file:///tmp/outside.png"));
    assert!(rebased.contains("https://example.com/test.png"));
    assert!(rebased.contains("data:image/png;base64,abcd"));
    assert!(rebased.contains("/usr/share/pixmaps/icon.png"));
}
