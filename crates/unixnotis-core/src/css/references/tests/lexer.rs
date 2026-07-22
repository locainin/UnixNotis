use super::super::lexer::{
    consume_escape, identifier_matches, skip_css_whitespace_and_comments, skip_quoted_value,
    trim_css_whitespace_range, valid_escape, would_start_identifier,
};
use super::super::{collect_css_import_values, collect_css_url_values, CssImportReference};

#[test]
fn escaped_url_function_names_are_decoded_at_every_identifier_position() {
    let css = r#"
        .a { background: u\72l("one.png"); }
        .b { background: U\000052L("two.png"); }
        .c { background: \75rl("three.png"); }
        .d { background: ur\6c("four.png"); }
        .e { background: u\rl("five.png"); }
        .f { background: u\000072 l("six.png"); }
    "#;

    let values = collect_css_url_values(css).expect("scan escaped URL names");

    assert_eq!(
        values
            .iter()
            .map(|reference| reference.value.as_str())
            .collect::<Vec<_>>(),
        [
            "one.png",
            "two.png",
            "three.png",
            "four.png",
            "five.png",
            "six.png",
        ]
    );
}

#[test]
fn escaped_import_keyword_and_url_function_are_both_decoded() {
    let quoted =
        collect_css_import_values(r#"@im\70ort "theme.css";"#).expect("scan escaped import");
    let urls = collect_css_url_values(r#"@im\70ort u\72l("nested.css");"#)
        .expect("scan escaped import URL");

    assert_eq!(
        quoted,
        vec![CssImportReference::Target("theme.css".to_string())]
    );
    assert_eq!(urls[0].value, "nested.css");
}

#[test]
fn escaped_reference_text_inside_comments_and_strings_stays_inactive() {
    let css = r#"
        /* u\72l("comment.png") */
        .a { content: 'U\000052L("string.png")'; }
        .b { background: url("real.png"); }
    "#;

    let values = collect_css_url_values(css).expect("scan active URL only");

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, "real.png");
}

#[test]
fn escaped_url_inside_an_image_function_is_still_discovered() {
    let css = r#".a { background-image: image(u\72l("nested.png")); }"#;

    let values = collect_css_url_values(css).expect("scan nested URL");

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, "nested.png");
}

#[test]
fn escaped_non_ascii_string_content_does_not_end_string_skipping_early() {
    let css = r#".a { content: "\é u\72l(fake.png)"; } .b { background: url(real.png); }"#;

    let values = collect_css_url_values(css).expect("scan after escaped Unicode string");

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, "real.png");
}

#[test]
fn fixed_identifier_matching_decodes_escapes_without_allocating_names() {
    assert_eq!(identifier_matches("u\\72l(", 0, "url"), (true, 5));
    assert_eq!(identifier_matches("im\\70ort ", 0, "import"), (true, 8));
    assert_eq!(identifier_matches("url-extra(", 0, "url"), (false, 9));
    assert_eq!(identifier_matches("éurl(", 0, "url"), (false, 5));
    assert_eq!(identifier_matches("xrl(", 0, "url"), (false, 3));
    assert_eq!(identifier_matches("urx(", 0, "url"), (false, 3));
}

#[test]
fn css_whitespace_range_trimming_handles_empty_and_nonempty_boundaries() {
    assert_eq!(trim_css_whitespace_range(b" value ", 0, 7), (1, 6));
    assert_eq!(trim_css_whitespace_range(b"  ", 0, 1), (1, 1));
    assert_eq!(trim_css_whitespace_range(b" x", 1, 1), (1, 1));
}

#[test]
fn hexadecimal_escape_consumption_is_limited_to_six_digits() {
    let (decoded, end) = consume_escape("\\1234567", 0);

    assert_eq!(decoded, '\u{FFFD}');
    assert_eq!(end, 7);
}

#[test]
fn escape_consumption_handles_each_css_newline_form_exactly() {
    assert_eq!(consume_escape("\\\r\nx", 0), ('\u{FFFD}', 3));
    assert_eq!(consume_escape("\\\nx", 0), ('\u{FFFD}', 2));
    assert_eq!(consume_escape("\\\rx", 0), ('\u{FFFD}', 2));
    assert_eq!(consume_escape("\\\u{000c}x", 0), ('\u{FFFD}', 2));
    assert_eq!(consume_escape("\\g\n", 0), ('g', 2));
}

#[test]
fn hexadecimal_escape_terminators_consume_one_css_whitespace_sequence() {
    assert_eq!(consume_escape("\\61\r\nx", 0), ('a', 5));
    assert_eq!(consume_escape("\\61\nx", 0), ('a', 4));
    assert_eq!(consume_escape("\\61\rx", 0), ('a', 4));
    assert_eq!(consume_escape("\\61\u{000c}x", 0), ('a', 4));
    assert_eq!(consume_escape("\\61 x", 0), ('a', 4));
    assert_eq!(consume_escape("\\61x", 0), ('a', 3));
}

#[test]
fn identifier_start_rules_reject_punctuation_and_malformed_escapes() {
    for accepted in ["a", "_", "-", "é", "\\a"] {
        assert!(
            would_start_identifier(accepted.as_bytes(), 0),
            "{accepted:?} should start an identifier"
        );
    }
    for rejected in ["", "0", ".", "\\", "\\\n", "\\\r", "\\\u{000c}"] {
        assert!(
            !would_start_identifier(rejected.as_bytes(), 0),
            "{rejected:?} should not start an identifier"
        );
    }
}

#[test]
fn escape_validation_requires_a_backslash_and_a_non_newline_byte() {
    assert!(valid_escape(b"\\a", 0));
    assert!(!valid_escape(b"aa", 0));
    assert!(!valid_escape(b"\\", 0));
    assert!(!valid_escape(b"\\\n", 0));
    assert!(!valid_escape(b"\\\r", 0));
    assert!(!valid_escape(b"\\\x0c", 0));
}

#[test]
fn whitespace_and_comment_skipping_returns_the_first_active_byte() {
    let input = b" \t/* first */  /* second */next";

    assert_eq!(skip_css_whitespace_and_comments(input, 0), 27);
    assert_eq!(skip_css_whitespace_and_comments(b"/*x*/next", 0), 5);
}

#[test]
fn broken_strings_stop_at_each_raw_css_newline_form() {
    assert_eq!(skip_quoted_value("\"broken\nnext", 0), Some(7));
    assert_eq!(skip_quoted_value("\"broken\rnext", 0), Some(7));
    assert_eq!(skip_quoted_value("\"broken\u{000c}next", 0), Some(7));
}

#[test]
fn broken_string_recovery_still_discovers_later_active_urls() {
    let css = ".a { content: \"broken\n} .b { background: u\\72l(real.png); }";

    let values = collect_css_url_values(css).expect("scan URL after broken string");

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, "real.png");
}
