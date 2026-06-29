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

#[test]
fn collect_url_spans_ignores_comment_bodies_and_handles_real_urls_afterward() {
    let css = "/* url(ignored.png) */\n.real { background: url(real.png); }";

    let spans = collect_url_spans(css);

    // Comment text can look like CSS, but it must not trigger asset rewrites
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].value, "real.png");
}

#[test]
fn collect_url_spans_stops_safely_on_unclosed_url() {
    let css = ".bad { background: url(unclosed.png";

    let spans = collect_url_spans(css);

    // Malformed trailing url(...) syntax should leave the original CSS untouched
    assert!(spans.is_empty());
}

#[test]
fn parse_url_value_trims_padding_and_strips_outer_quotes() {
    let css = "url(  \"icons/a.png\"  )";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, next_index) = parse_url_value(css, open_index).expect("url value");

    // Stored values are normalized enough for path rebasing while byte ranges stay exact
    assert_eq!(span.value, "icons/a.png");
    assert_eq!(next_index, css.len());
    assert_eq!(&css[span.value_start..span.value_end], "icons/a.png");
}

#[test]
fn parse_url_value_preserves_inner_quotes_in_malformed_unquoted_value() {
    let css = "url(icon\"bad'.png)";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, _) = parse_url_value(css, open_index).expect("url value");

    // Odd but recoverable CSS should remain readable instead of dropping user bytes
    assert_eq!(span.value, "icon\"bad'.png");
}

#[test]
fn normalize_lexical_path_preserves_leading_parent_segments() {
    let normalized = normalize_lexical_path(Path::new("../assets/./icons/../icon.png"));

    // Relative paths outside the base still keep the leading parent segment
    assert_eq!(normalized, Path::new("../assets/icon.png"));
}

#[test]
fn normalize_lexical_path_does_not_pop_past_root() {
    let normalized = normalize_lexical_path(Path::new("/tmp/../../icon.png"));

    // Absolute paths must never collapse into a relative path while folding parents
    assert_eq!(normalized, Path::new("/icon.png"));
}

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
