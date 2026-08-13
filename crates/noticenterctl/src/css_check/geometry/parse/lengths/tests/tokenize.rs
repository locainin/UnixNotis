use super::{
    consume_balanced_group, split_css_value_tokens, split_top_level_list, split_top_level_once,
    CssScanError,
};

#[test]
fn escaped_quotes_keep_separators_inside_the_same_string() {
    let value = r#"var(--label, "quoted\",comma"), 12px"#;

    assert_eq!(
        split_top_level_list(value, ',').expect("scan escaped quote"),
        vec![r#"var(--label, "quoted\",comma")"#, "12px"]
    );
}

#[test]
fn escaped_whitespace_does_not_split_a_css_value_token() {
    assert_eq!(
        split_css_value_tokens(r"10px label\ value 20px").expect("scan escaped space"),
        vec!["10px", r"label\ value", "20px"]
    );
}

#[test]
fn balanced_group_returns_the_byte_after_its_matching_parenthesis() {
    let value = "calc(10px + var(--gap, 2px)) tail";
    let start = value.find('(').expect("opening parenthesis");

    assert_eq!(
        consume_balanced_group(value, start),
        Some("calc(10px + var(--gap, 2px))".len())
    );
}

#[test]
fn top_level_once_ignores_nested_and_quoted_separators() {
    let value = r#"--gap, min(10px, "20px,still-string")"#;

    assert_eq!(
        split_top_level_once(value, ',').expect("scan var fallback"),
        ("--gap", Some(r#" min(10px, "20px,still-string")"#))
    );
}

#[test]
fn top_level_once_returns_no_fallback_when_separator_is_absent() {
    assert_eq!(
        split_top_level_once("--gap", ',').expect("scan value without fallback"),
        ("--gap", None)
    );
}

#[test]
fn bracketed_separators_remain_inside_their_value() {
    assert_eq!(
        split_top_level_list("selector[data='a,b'], 12px", ',').expect("scan bracketed selector"),
        vec!["selector[data='a,b']", "12px"]
    );
    assert_eq!(
        split_css_value_tokens("selector[data=value with-space] 12px")
            .expect("scan bracketed whitespace"),
        vec!["selector[data=value with-space]", "12px"]
    );
}

#[test]
fn malformed_delimiters_and_strings_return_structured_errors() {
    assert_eq!(
        split_top_level_list("10px), 20px", ','),
        Err(CssScanError::ClosingParenthesis(4))
    );
    assert_eq!(
        split_css_value_tokens(r#"10px "unfinished"#),
        Err(CssScanError::UnterminatedQuote)
    );
    assert_eq!(
        split_css_value_tokens("calc(10px"),
        Err(CssScanError::UnterminatedGroup)
    );
    assert_eq!(
        split_css_value_tokens("selector[value"),
        Err(CssScanError::UnterminatedGroup)
    );
    assert_eq!(
        split_css_value_tokens("selector]"),
        Err(CssScanError::ClosingBracket(8))
    );
    assert_eq!(
        split_css_value_tokens("value\\"),
        Err(CssScanError::DanglingEscape)
    );
}
