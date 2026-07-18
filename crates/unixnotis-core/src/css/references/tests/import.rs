use super::super::{
    collect_css_import_dependency_values, collect_css_import_url_spans, collect_css_import_values,
    CssImportReference,
};

#[test]
fn import_scanner_ignores_comments_strings_and_url_forms_for_asset_collection() {
    let css = "/* @import \"comment.css\"; */\n\
               .a { content: '@import \"string.css\";'; }\n\
               @import url(\"url-form.css\");\n\
               @import \"real.css\";\n";

    let imports = collect_css_import_values(css).expect("scan imports");

    assert_eq!(
        imports,
        vec![CssImportReference::Target("real.css".to_string())]
    );
}

#[test]
fn import_url_spans_match_only_url_form_payload_ranges() {
    let css = r#"@import "plain.css"; @im\70ort u\72l("escaped.css");"#;

    let spans = collect_css_import_url_spans(css).expect("scan import URL ranges");

    assert_eq!(spans.len(), 1);
    assert_eq!(
        &css[spans[0].value_start..spans[0].value_end],
        "escaped.css"
    );
}

#[test]
fn dependency_scanner_includes_plain_and_escaped_url_forms() {
    let css = r#"@IMPORT URL("upper.css"); @im\70ort/* token gap */u\72l('escaped.css');"#;

    let imports = collect_css_import_dependency_values(css).expect("scan dependencies");

    assert_eq!(
        imports,
        vec![
            CssImportReference::Target("upper.css".to_string()),
            CssImportReference::Target("escaped.css".to_string()),
        ]
    );
}

#[test]
fn dynamic_and_escaped_import_payloads_are_ambiguous() {
    let css = "@import var(--theme); @import \"\\2f tmp/theme.css\";";

    let imports = collect_css_import_values(css).expect("scan ambiguous imports");

    assert_eq!(
        imports,
        vec![CssImportReference::Ambiguous, CssImportReference::Ambiguous]
    );
}

#[test]
fn import_count_is_bounded() {
    let imports = "@import \"a.css\";".repeat(4_097);

    let error = collect_css_import_values(&imports).expect_err("reject excess imports");

    assert!(error
        .to_string()
        .contains("more than 4096 import references"));
}

#[test]
fn semicolons_inside_quoted_targets_do_not_end_import_statements() {
    let css = r#"@import "theme;.css"; @import "next.css";"#;

    let imports = collect_css_import_values(css).expect("scan imports containing semicolons");

    assert_eq!(
        imports,
        vec![
            CssImportReference::Target("theme;.css".to_string()),
            CssImportReference::Target("next.css".to_string()),
        ]
    );
}

#[test]
fn broken_import_string_does_not_hide_the_next_import() {
    let css = "@import \"broken\n@import \"next.css\";";

    let imports = collect_css_import_values(css).expect("scan import after broken string");

    assert_eq!(
        imports,
        vec![
            CssImportReference::Ambiguous,
            CssImportReference::Target("next.css".to_string()),
        ]
    );
}
