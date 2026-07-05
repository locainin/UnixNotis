use super::{
    next_css_block_with_offsets, parse_css_declarations_with_offsets, should_recurse_at_rule,
    split_selectors, strip_css_comments,
};

#[test]
fn strip_css_comments_preserves_byte_count_and_line_count() {
    let css = ".a { color: red; }\n/* hidden\nrule */\n.b { color: blue; }";

    let stripped = strip_css_comments(css);

    assert_eq!(stripped.len(), css.len());
    assert_eq!(stripped.matches('\n').count(), css.matches('\n').count());
    assert!(!stripped.contains("hidden"));
    assert!(stripped.contains(".b { color: blue; }"));
}

#[test]
fn next_css_block_with_offsets_ignores_braces_inside_strings() {
    let css = ".a { content: \"{\"; } .b { color: red; }";

    let block = next_css_block_with_offsets(css.as_bytes(), 0).expect("first block");

    assert_eq!(block.selector.trim(), ".a");
    assert_eq!(block.block.trim(), "content: \"{\";");
    assert_eq!(&css[block.selector_start..block.selector_start + 2], ".a");

    let second = next_css_block_with_offsets(css.as_bytes(), block.next).expect("second block");
    assert_eq!(second.selector.trim(), ".b");
    assert_eq!(second.block.trim(), "color: red;");
    assert_eq!(
        second.block_start + second.block.find("color").expect("color offset"),
        css.find("color").expect("source color offset")
    );
}

#[test]
fn parse_css_declarations_keeps_nested_colons_and_semicolons_inside_values() {
    let block = r#"background: url("data:image/svg+xml;utf8,<svg></svg>"); content: "a:b;c"; padding: calc(4px + 2px);"#;

    let declarations = parse_css_declarations_with_offsets(block);

    assert_eq!(declarations.len(), 3);
    assert_eq!(declarations[0].name, "background");
    assert!(declarations[0].value.contains("data:image/svg+xml;utf8"));
    assert_eq!(declarations[1].value, r#""a:b;c""#);
    assert_eq!(declarations[2].value, "calc(4px + 2px)");
    assert_eq!(
        declarations[2].start,
        block.find("padding").expect("padding offset")
    );
}

#[test]
fn split_selectors_only_splits_top_level_commas() {
    let selector = r#".a:is(.b, .c), .d[data-label="x,y"], .e:not(.f, .g)"#;

    let selectors = split_selectors(selector);

    assert_eq!(
        selectors,
        vec![
            ".a:is(.b, .c)".to_string(),
            r#".d[data-label="x,y"]"#.to_string(),
            ".e:not(.f, .g)".to_string(),
        ]
    );
}

#[test]
fn should_recurse_at_rule_accepts_selector_bearing_at_rules_only() {
    assert!(should_recurse_at_rule("@media (min-width: 1px)"));
    assert!(should_recurse_at_rule("@supports (color: red)"));
    assert!(should_recurse_at_rule("@layer theme"));
    assert!(should_recurse_at_rule("@container panel (width > 1px)"));
    assert!(should_recurse_at_rule("@document url-prefix(\"app\")"));

    assert!(!should_recurse_at_rule("@keyframes pulse"));
    assert!(!should_recurse_at_rule("@font-face"));
}
