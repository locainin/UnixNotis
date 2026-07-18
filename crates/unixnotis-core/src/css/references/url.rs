//! Decoded CSS `url(...)` discovery and byte-range extraction

use super::lexer::{
    consume_escape, consume_identifier, skip_comment, skip_css_whitespace, skip_quoted_value,
    starts_comment, utf8_char_len, would_start_identifier,
};
use super::{CssReference, CssReferenceError, CssUrlSpan};

pub(super) const MAX_CSS_REFERENCES_PER_FILE: usize = 4_096;

/// Collect every decoded `url(...)` payload and its source range
///
/// # Errors
///
/// Returns an error when a URL is incomplete or the per-file reference limit is exceeded
pub fn collect_css_url_spans(css_text: &str) -> Result<Vec<CssUrlSpan>, CssReferenceError> {
    let bytes = css_text.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;

    // Valid scans advance at least one source byte per pass
    for _ in 0..bytes.len().saturating_add(1) {
        if index >= bytes.len() {
            return Ok(spans);
        }
        if starts_comment(bytes, index) {
            // Comment contents are not CSS tokens and must never produce references
            let next_index = skip_comment(bytes, index).unwrap_or(bytes.len());
            if next_index <= index {
                return Err(CssReferenceError::ScannerDidNotAdvance);
            }
            index = next_index;
            continue;
        }
        if matches!(bytes[index], b'\'' | b'"') {
            // Strings can contain documentation text that resembles active CSS
            let next_index = skip_quoted_value(css_text, index).unwrap_or(bytes.len());
            if next_index <= index {
                return Err(CssReferenceError::ScannerDidNotAdvance);
            }
            index = next_index;
            continue;
        }
        if !would_start_identifier(bytes, index) {
            index = index.saturating_add(1);
            continue;
        }

        // CSS escapes are decoded while the source indexes remain byte-exact
        let (name, name_end) = consume_identifier(css_text, index);
        if name.eq_ignore_ascii_case("url") && bytes.get(name_end) == Some(&b'(') {
            let (span, next_index) = parse_url_value(css_text, name_end.saturating_add(1))
                .ok_or(CssReferenceError::UnterminatedUrl)?;
            if next_index <= index {
                return Err(CssReferenceError::ScannerDidNotAdvance);
            }
            if spans.len() >= MAX_CSS_REFERENCES_PER_FILE {
                return Err(CssReferenceError::TooManyUrls(MAX_CSS_REFERENCES_PER_FILE));
            }
            spans.push(span);
            index = next_index;
            continue;
        }

        // A valid identifier start always consumes at least one source byte
        if name_end <= index {
            return Err(CssReferenceError::ScannerDidNotAdvance);
        }
        index = name_end;
    }

    Err(CssReferenceError::ScannerDidNotAdvance)
}

/// Collect every decoded `url(...)` payload without source ranges
///
/// # Errors
///
/// Returns the same bounded scanner errors as [`collect_css_url_spans`]
pub fn collect_css_url_values(css_text: &str) -> Result<Vec<CssReference>, CssReferenceError> {
    Ok(collect_css_url_spans(css_text)?
        .into_iter()
        .map(|span| CssReference {
            value: span.value,
            ambiguous: span.ambiguous,
        })
        .collect())
}

pub(super) fn parse_url_value(input: &str, open_index: usize) -> Option<(CssUrlSpan, usize)> {
    let bytes = input.as_bytes();
    let mut index = skip_css_whitespace(bytes, open_index);
    if index < open_index || index >= bytes.len() {
        return None;
    }

    let mut ambiguous = false;
    if let quote @ (b'\'' | b'"') = bytes[index] {
        let value_start = index.saturating_add(1);
        index = value_start;
        for _ in 0..bytes.len().saturating_add(1) {
            let Some(&byte) = bytes.get(index) else {
                break;
            };
            if byte == b'\\' {
                ambiguous = true;
                let next_index = consume_escape(input, index).1;
                if next_index <= index {
                    return None;
                }
                index = next_index;
                continue;
            }
            if byte == quote {
                let value_end = index;
                index = skip_css_whitespace(bytes, index.saturating_add(1));
                if bytes.get(index) != Some(&b')') {
                    return None;
                }
                return Some((
                    CssUrlSpan {
                        value: input[value_start..value_end].to_string(),
                        value_start,
                        value_end,
                        ambiguous,
                    },
                    index.saturating_add(1),
                ));
            }
            if matches!(byte, b'\n' | b'\r' | b'\x0c') {
                // Raw newlines cannot occur inside a valid quoted URL token
                return None;
            }
            index = index.saturating_add(utf8_char_len(byte));
        }
        return None;
    }

    let raw_start = index;
    let mut saw_unquoted_whitespace = false;
    for _ in 0..bytes.len().saturating_add(1) {
        let Some(&byte) = bytes.get(index) else {
            break;
        };
        if byte == b'\\' {
            ambiguous = true;
            index = consume_escape(input, index).1;
            continue;
        }
        if byte == b')' {
            let raw = &input[raw_start..index];
            let value = raw.trim();
            // Leading whitespace was consumed before raw_start was recorded
            let value_start = raw_start;
            let value_end = value_start + value.len();
            return Some((
                CssUrlSpan {
                    value: value.to_string(),
                    value_start,
                    value_end,
                    ambiguous,
                },
                index.saturating_add(1),
            ));
        }
        if matches!(byte, b'\'' | b'"') || byte.is_ascii_control() {
            // Invalid unquoted payload syntax remains visible to fail-closed callers
            ambiguous = true;
        }
        if byte.is_ascii_whitespace() {
            saw_unquoted_whitespace = true;
        } else if saw_unquoted_whitespace {
            // Only whitespace directly before the closing parenthesis is valid here
            ambiguous = true;
        }
        index = index.saturating_add(utf8_char_len(byte));
    }
    None
}
