use super::{
    collect_import_dependency_values, collect_import_values, collect_url_spans, collect_url_values,
    parse_import_value, parse_url_value, skip_comment, skip_quoted_value, starts_with_import,
    strip_css_comments, CssImportReference,
};

#[test]
fn import_parser_ignores_comments_strings_and_url_forms() {
    let css = "/* @import \"comment.css\"; */\n\
               .a { content: '@import \"string.css\";'; }\n\
               @import url(\"url-form.css\");\n\
               @import \"real.css\";\n";

    let imports = collect_import_values(css).expect("scan imports");

    assert_eq!(imports.len(), 1);
    assert!(matches!(
        imports.first(),
        Some(CssImportReference::Target(value)) if value == "real.css"
    ));
}

#[test]
fn import_parser_does_not_confuse_slashes_or_other_at_rules_with_imports() {
    let css = ".a { width: calc(1/2); }\n\
               @media screen { .a { color: red; } }\n\
               prefix/* comment with * and / inside */\n\
               @import \"real.css\";";

    let imports = collect_import_values(css).expect("scan import after unrelated syntax");

    assert_eq!(imports.len(), 1);
    assert!(matches!(
        imports.first(),
        Some(CssImportReference::Target(value)) if value == "real.css"
    ));
}

#[test]
fn import_parser_keeps_scanning_after_an_ordinary_slash() {
    let imports = collect_import_values(".a { width: calc(1/2); } @import \"real.css\";")
        .expect("scan import after slash");

    assert_eq!(imports.len(), 1);
    assert!(matches!(
        imports.first(),
        Some(CssImportReference::Target(value)) if value == "real.css"
    ));
}

#[test]
fn import_parser_marks_dynamic_and_escaped_forms_ambiguous() {
    let css = "@import var(--theme);\n@import \"\\2f tmp/theme.css\";\n";

    let imports = collect_import_values(css).expect("scan imports");

    assert_eq!(imports.len(), 2);
    assert!(imports
        .iter()
        .all(|reference| matches!(reference, CssImportReference::Ambiguous)));
}

#[test]
fn url_parser_marks_css_escapes_ambiguous() {
    let references =
        collect_url_values(".a { background: url(\"\\2f tmp/image.png\"); }").expect("scan URLs");

    assert_eq!(references.len(), 1);
    assert!(references[0].ambiguous);
}

#[test]
fn scanners_reject_reference_counts_over_the_per_file_limit() {
    let urls = "url(a)".repeat(4_097);
    let imports = "@import \"a.css\";".repeat(4_097);

    assert!(collect_url_values(&urls)
        .expect_err("URL count must stay bounded")
        .to_string()
        .contains("more than 4096 URL references"));
    assert!(collect_import_values(&imports)
        .expect_err("import count must stay bounded")
        .to_string()
        .contains("more than 4096 import references"));
}

#[test]
fn scanners_accept_the_exact_per_file_reference_limit() {
    let urls = "url(a)".repeat(4_096);
    let imports = "@import \"a.css\";".repeat(4_096);

    assert_eq!(
        collect_url_values(&urls).expect("exact URL limit").len(),
        4_096
    );
    assert_eq!(
        collect_import_values(&imports)
            .expect("exact import limit")
            .len(),
        4_096
    );
}

#[test]
fn url_parser_preserves_ranges_quotes_spacing_and_case() {
    let css = "URL( one.png ) url(\"two.png\") url('three.png')";
    let spans = collect_url_spans(css).expect("scan URL forms");

    assert_eq!(spans.len(), 3);
    assert_eq!(
        spans
            .iter()
            .map(|span| span.value.as_str())
            .collect::<Vec<_>>(),
        ["one.png", "two.png", "three.png"]
    );
    assert_eq!(&css[spans[0].value_start..spans[0].value_end], "one.png");
    assert_eq!(&css[spans[1].value_start..spans[1].value_end], "two.png");
    assert_eq!(&css[spans[2].value_start..spans[2].value_end], "three.png");
}

#[test]
fn url_scanner_ignores_comments_strings_and_identifier_suffixes() {
    let css = "prefix/* slash / url(comment.png) */\
               .a { content: \"url(string.png)\"; }\
               .b { value: myurl(identifier.png); }\
               .c { background: url(real.png); }";

    let references = collect_url_values(css).expect("scan real URL only");

    assert_eq!(references.len(), 1);
    assert_eq!(references[0].value, "real.png");
}

#[test]
fn url_scanner_keeps_scanning_after_an_ordinary_slash() {
    let references = collect_url_values(".a { width: calc(1/2); background: url(real.png); }")
        .expect("scan URL after slash");

    assert_eq!(references.len(), 1);
    assert_eq!(references[0].value, "real.png");
}

#[test]
fn url_scanner_rejects_unterminated_references() {
    for css in [
        "url(",
        "url(  ",
        "url(\"missing quote)",
        "url(missing close",
    ] {
        assert!(collect_url_values(css)
            .expect_err("unterminated URL must fail closed")
            .to_string()
            .contains("unterminated"));
    }
}

#[test]
fn import_keyword_requires_a_complete_at_rule_name() {
    for valid in [
        b"@import".as_slice(),
        b"@IMPORT ".as_slice(),
        b"@import;".as_slice(),
    ] {
        assert!(starts_with_import(valid, 0));
    }
    for other in [
        b"@important".as_slice(),
        b"@imported".as_slice(),
        b"@import-name".as_slice(),
        b"@import_name".as_slice(),
        b"@impor".as_slice(),
    ] {
        assert!(!starts_with_import(other, 0));
    }
}

#[test]
fn import_value_parser_handles_both_quotes_and_statement_boundaries() {
    let double = "@import  \"double.css\" screen;next";
    let single = "@import 'single.css';next";

    let (double_ref, double_end) = parse_import_value(double, 7, false);
    let (single_ref, single_end) = parse_import_value(single, 7, false);

    assert!(matches!(
        double_ref,
        Some(CssImportReference::Target(value)) if value == "double.css"
    ));
    assert_eq!(&double[double_end..], "next");
    assert!(matches!(
        single_ref,
        Some(CssImportReference::Target(value)) if value == "single.css"
    ));
    assert_eq!(&single[single_end..], "next");
}

#[test]
fn import_statement_end_ignores_semicolons_inside_the_quoted_target() {
    let input = "@import \"semi;colon.css\";next";

    let (reference, next) = parse_import_value(input, 7, false);

    assert!(matches!(
        reference,
        Some(CssImportReference::Target(value)) if value == "semi;colon.css"
    ));
    assert_eq!(&input[next..], "next");
}

#[test]
fn dependency_import_parser_includes_url_forms_without_duplicate_asset_findings() {
    let css = "@IMPORT URL(\"upper.css\");\n@import url('single.css');";

    assert!(collect_import_values(css)
        .expect("asset imports")
        .is_empty());
    let dependencies = collect_import_dependency_values(css).expect("dependency imports");
    assert_eq!(dependencies.len(), 2);
    assert!(matches!(
        dependencies.first(),
        Some(CssImportReference::Target(value)) if value == "upper.css"
    ));
    assert!(matches!(
        dependencies.get(1),
        Some(CssImportReference::Target(value)) if value == "single.css"
    ));
}

#[test]
fn parser_helpers_stop_at_exact_quote_comment_and_url_boundaries() {
    assert_eq!(skip_quoted_value(b"\"a\\\"b\"tail", 0), Some(6));
    assert_eq!(skip_quoted_value(b"'abc", 0), None);
    assert_eq!(skip_comment(b"body*/tail", 0), Some(6));
    assert_eq!(skip_comment(b"body", 0), None);

    let input = "url( value )tail";
    let (span, next) = parse_url_value(input, 4).expect("parse URL payload");
    assert_eq!(span.value, "value");
    assert_eq!(&input[next..], "tail");

    let quoted = "url(  \"quoted.png\"   )tail";
    let (span, next) = parse_url_value(quoted, 4).expect("parse spaced quoted URL");
    assert_eq!(span.value, "quoted.png");
    assert_eq!(&quoted[span.value_start..span.value_end], "quoted.png");
    assert_eq!(&quoted[next..], "tail");
}

#[test]
fn comment_stripping_handles_complete_and_unterminated_comments() {
    assert_eq!(
        strip_css_comments("a/* hidden * and / still hidden */b/* tail").as_ref(),
        "ab"
    );
    let unchanged = "a / b";
    assert!(matches!(
        strip_css_comments(unchanged),
        std::borrow::Cow::Borrowed(value) if value == unchanged
    ));
}
