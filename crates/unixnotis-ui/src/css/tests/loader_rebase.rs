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
