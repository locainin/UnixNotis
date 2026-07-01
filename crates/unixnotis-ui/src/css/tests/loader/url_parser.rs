use super::*;

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
fn parse_url_value_keeps_trailing_text_after_closed_quote_readable() {
    let css = "url(\"icon.svg\"fallback)";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, _) = parse_url_value(css, open_index).expect("url value");

    assert_eq!(span.value, "icon.svgfallback");
    assert_eq!(&css[span.value_start..span.value_end], "icon.svg\"fallback");
}

#[test]
fn parse_url_value_ignores_closed_quote_padding_before_malformed_suffix() {
    let css = "url(\"icon.svg\"  fallback with space)";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, _) = parse_url_value(css, open_index).expect("url value");

    // Only padding before the malformed suffix is ignored; suffix spaces remain user text
    assert_eq!(span.value, "icon.svgfallback with space");
    assert_eq!(
        &css[span.value_start..span.value_end],
        "icon.svg\"  fallback with space"
    );
}

#[test]
fn parse_url_value_preserves_unicode_quoted_path() {
    let css = "url(\"icons/café.png\")";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, next_index) = parse_url_value(css, open_index).expect("url value");

    // URL parsing must keep UTF-8 characters intact while still returning byte indexes
    assert_eq!(span.value, "icons/café.png");
    assert_eq!(&css[span.value_start..span.value_end], "icons/café.png");
    assert_eq!(next_index, css.len());
}

#[test]
fn parse_url_value_rejects_unclosed_quoted_payload() {
    let css = "url(\"icons/a.png)";
    let open_index = css.find('(').expect("url open") + 1;

    assert!(parse_url_value(css, open_index).is_none());
}

#[test]
fn parse_url_value_preserves_unquoted_quotes_as_payload_bytes() {
    let css = "url(icon'odd\"name.svg)";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, next_index) = parse_url_value(css, open_index).expect("url value");

    assert_eq!(span.value, "icon'odd\"name.svg");
    assert_eq!(next_index, css.len());
}

#[test]
fn parse_url_value_keeps_unquoted_trailing_padding_outside_replacement_range() {
    let css = "url(icons/a.png   )";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, _) = parse_url_value(css, open_index).expect("url value");

    assert_eq!(span.value, "icons/a.png");
    assert_eq!(&css[span.value_start..span.value_end], "icons/a.png");
    assert_eq!(&css[span.value_end..], "   )");
}

#[test]
fn parse_url_value_preserves_inner_quotes_in_malformed_unquoted_value() {
    let css = "url(icon\"bad'.png)";
    let open_index = css.find('(').expect("url open") + 1;

    let (span, _) = parse_url_value(css, open_index).expect("url value");

    // Odd but recoverable CSS should remain readable instead of dropping user bytes
    assert_eq!(span.value, "icon\"bad'.png");
}
