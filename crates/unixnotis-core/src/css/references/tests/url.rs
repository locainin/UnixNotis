use super::super::url::parse_url_value;
use super::super::{collect_css_url_spans, collect_css_url_values};

#[test]
fn url_scanner_preserves_ranges_quotes_spacing_and_case() {
    let css = "URL( one.png ) url(\"two.png\") url('three.png')";

    let spans = collect_css_url_spans(css).expect("scan URL forms");

    assert_eq!(spans.len(), 3);
    assert_eq!(&css[spans[0].value_start..spans[0].value_end], "one.png");
    assert_eq!(&css[spans[1].value_start..spans[1].value_end], "two.png");
    assert_eq!(&css[spans[2].value_start..spans[2].value_end], "three.png");
}

#[test]
fn url_payload_escapes_are_reported_as_ambiguous() {
    let values = collect_css_url_values(
        r#".a { background: url("\2f tmp/image.png"); mask: url(icon\20one.png); }"#,
    )
    .expect("scan escaped payload");

    assert_eq!(values.len(), 2);
    assert!(values.iter().all(|value| value.ambiguous));
}

#[test]
fn unterminated_url_forms_fail_closed() {
    for css in [
        "url(",
        "url(  ",
        "url(\"missing quote)",
        "url(missing close",
    ] {
        let error = collect_css_url_values(css).expect_err("reject incomplete URL");
        assert!(error.to_string().contains("unterminated"));
    }
}

#[test]
fn url_count_is_bounded() {
    let urls = "url(a)".repeat(4_097);

    let error = collect_css_url_values(&urls).expect_err("reject excess URLs");

    assert!(error.to_string().contains("more than 4096 URL references"));
}

#[test]
fn comments_at_the_start_of_input_are_skipped_before_active_urls() {
    let css = "/* url(hidden.png) */url(active.png)";

    let values = collect_css_url_values(css).expect("scan URL after leading comment");

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, "active.png");
}

#[test]
fn parsed_url_cursor_points_after_the_closing_parenthesis() {
    let quoted = "  \"image.png\"  )tail";
    let unquoted = "image.png)tail";

    let (quoted_span, quoted_end) = parse_url_value(quoted, 0).expect("parse quoted URL");
    let (unquoted_span, unquoted_end) = parse_url_value(unquoted, 0).expect("parse unquoted URL");

    assert_eq!(quoted_span.value, "image.png");
    assert_eq!(quoted_span.value_start, 3);
    assert_eq!(quoted_span.value_end, 12);
    assert_eq!(quoted_end, 16);
    assert_eq!(unquoted_span.value, "image.png");
    assert_eq!(unquoted_span.value_start, 0);
    assert_eq!(unquoted_span.value_end, 9);
    assert_eq!(unquoted_end, 10);
}

#[test]
fn invalid_unquoted_delimiters_and_controls_are_marked_ambiguous() {
    for css in [
        "url(image\"name.png)",
        "url(image'name.png)",
        "url(image name.png)",
        "url(image\tname.png)",
    ] {
        let values = collect_css_url_values(css).expect("scan invalid unquoted payload");

        assert_eq!(values.len(), 1);
        assert!(values[0].ambiguous, "{css:?} should be ambiguous");
    }
}
