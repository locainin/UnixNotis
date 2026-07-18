use super::{
    collect_import_dependency_values, collect_import_values, collect_url_spans, collect_url_values,
    CssImportReference,
};

#[test]
fn preset_scanner_uses_decoded_function_and_at_keyword_names() {
    let css = r#"
        .a { background: u\72l("file:///outside.png"); }
        .b { background: U\000052L("https://example.invalid/a.png"); }
        @im\70ort "outside.css";
    "#;

    let urls = collect_url_values(css).expect("scan escaped URL names");
    let imports = collect_import_values(css).expect("scan escaped import name");

    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0].value, "file:///outside.png");
    assert_eq!(urls[1].value, "https://example.invalid/a.png");
    assert_eq!(
        imports,
        vec![CssImportReference::Target("outside.css".to_string())]
    );
}

#[test]
fn preset_scanner_keeps_source_ranges_for_rewriting() {
    let css = "URL( one.png ) u\\72l(\"two.png\")";

    let spans = collect_url_spans(css).expect("scan URL ranges");

    assert_eq!(spans.len(), 2);
    assert_eq!(&css[spans[0].value_start..spans[0].value_end], "one.png");
    assert_eq!(&css[spans[1].value_start..spans[1].value_end], "two.png");
}

#[test]
fn preset_scanner_marks_escaped_payloads_ambiguous() {
    let urls = collect_url_values(r#"url("\2f tmp/image.png")"#).expect("scan escaped URL payload");
    let imports = collect_import_values(r#"@import "\2f tmp/theme.css";"#)
        .expect("scan escaped import payload");

    assert!(urls[0].ambiguous);
    assert_eq!(imports, vec![CssImportReference::Ambiguous]);
}

#[test]
fn dependency_scanner_includes_escaped_url_import_forms() {
    let imports = collect_import_dependency_values(r#"@im\70ort u\72l("theme.css");"#)
        .expect("scan escaped dependency");

    assert_eq!(
        imports,
        vec![CssImportReference::Target("theme.css".to_string())]
    );
}
