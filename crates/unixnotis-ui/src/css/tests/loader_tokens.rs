use std::path::Path;

use super::*;

#[test]
fn ensure_base_tokens_adds_missing_tokens_once() {
    let path = Path::new("/tmp/unixnotis/base.css");
    let contents = ".panel { color: @unixnotis-surface; }";

    let first = ensure_base_tokens(contents, path);
    let second = ensure_base_tokens(&first, path);

    // Missing legacy base tokens are injected, but a second pass should not duplicate them
    assert!(first.contains("@define-color unixnotis-surface-base"));
    assert!(first.contains("@define-color unixnotis-card-base"));
    assert_eq!(first, second);
}

#[test]
fn ensure_base_tokens_injects_when_only_one_required_token_is_present() {
    let path = Path::new("/tmp/unixnotis/base.css");
    let contents = "@define-color unixnotis-surface-base #111;";

    let ensured = ensure_base_tokens(contents, path);

    assert!(ensured.starts_with("@define-color unixnotis-surface-base @unixnotis-surface;"));
    assert!(ensured.contains("@define-color unixnotis-card-base @unixnotis-card;"));
    assert!(ensured.ends_with(contents));
}

#[test]
fn ensure_base_tokens_keeps_complete_stylesheet_unchanged() {
    let contents = concat!(
        "@define-color unixnotis-surface-base #111;\n",
        "@define-color unixnotis-card-base #222;\n",
        ".panel { color: @unixnotis-surface-base; }",
    );

    let ensured = ensure_base_tokens(contents, Path::new("/tmp/base.css"));

    // Complete base files should stay byte-for-byte stable
    assert_eq!(ensured, contents);
}
