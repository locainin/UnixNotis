//! Decoded CSS `@import` discovery

use super::lexer::{
    consume_escape, consume_identifier, skip_comment, skip_css_whitespace_and_comments,
    skip_quoted_value, starts_comment, utf8_char_len, would_start_identifier,
};
use super::url::{parse_url_value, MAX_CSS_REFERENCES_PER_FILE};
use super::{CssImportReference, CssReferenceError, CssUrlSpan};

struct ImportRecord {
    reference: CssImportReference,
    url_span: Option<CssUrlSpan>,
}

/// Collect quoted `@import` targets while leaving URL forms to the URL scanner
///
/// # Errors
///
/// Returns an error when the per-file import limit is exceeded
pub fn collect_css_import_values(
    css_text: &str,
) -> Result<Vec<CssImportReference>, CssReferenceError> {
    Ok(collect_import_records(css_text)?
        .into_iter()
        // URL forms are already covered by the general URL reference scanner
        .filter(|record| record.url_span.is_none())
        .map(|record| record.reference)
        .collect())
}

/// Collect quoted and `url(...)` import targets for dependency traversal
///
/// # Errors
///
/// Returns an error when the per-file import limit is exceeded
pub fn collect_css_import_dependency_values(
    css_text: &str,
) -> Result<Vec<CssImportReference>, CssReferenceError> {
    Ok(collect_import_records(css_text)?
        .into_iter()
        .map(|record| record.reference)
        .collect())
}

/// Collect exact payload ranges for `@import url(...)` references
///
/// # Errors
///
/// Returns an error when the per-file import limit is exceeded
pub fn collect_css_import_url_spans(css_text: &str) -> Result<Vec<CssUrlSpan>, CssReferenceError> {
    Ok(collect_import_records(css_text)?
        .into_iter()
        .filter_map(|record| record.url_span)
        .collect())
}

fn collect_import_records(css_text: &str) -> Result<Vec<ImportRecord>, CssReferenceError> {
    let bytes = css_text.as_bytes();
    let mut imports = Vec::new();
    let mut index = 0usize;

    // Valid scans advance at least one source byte per pass
    for _ in 0..bytes.len().saturating_add(1) {
        if index >= bytes.len() {
            return Ok(imports);
        }
        if starts_comment(bytes, index) {
            let next_index = skip_comment(bytes, index).unwrap_or(bytes.len());
            if next_index <= index {
                return Err(CssReferenceError::ScannerDidNotAdvance);
            }
            index = next_index;
            continue;
        }
        if matches!(bytes[index], b'\'' | b'"') {
            let next_index = skip_quoted_value(css_text, index).unwrap_or(bytes.len());
            if next_index <= index {
                return Err(CssReferenceError::ScannerDidNotAdvance);
            }
            index = next_index;
            continue;
        }
        if bytes[index] != b'@' || !would_start_identifier(bytes, index.saturating_add(1)) {
            index = index.saturating_add(1);
            continue;
        }

        // At-keyword names follow the same escape rules as function identifiers
        let (name, name_end) = consume_identifier(css_text, index.saturating_add(1));
        if !name.eq_ignore_ascii_case("import") {
            if name_end <= index {
                return Err(CssReferenceError::ScannerDidNotAdvance);
            }
            index = name_end;
            continue;
        }

        let (reference, url_span, next_index) = parse_import_value(css_text, name_end);
        if let Some(reference) = reference {
            if imports.len() >= MAX_CSS_REFERENCES_PER_FILE {
                return Err(CssReferenceError::TooManyImports(
                    MAX_CSS_REFERENCES_PER_FILE,
                ));
            }
            imports.push(ImportRecord {
                reference,
                url_span,
            });
        }
        let advanced_index = next_index.max(name_end);
        if advanced_index <= index {
            return Err(CssReferenceError::ScannerDidNotAdvance);
        }
        index = advanced_index;
    }

    Err(CssReferenceError::ScannerDidNotAdvance)
}

fn parse_import_value(
    input: &str,
    start: usize,
) -> (Option<CssImportReference>, Option<CssUrlSpan>, usize) {
    let bytes = input.as_bytes();
    let mut index = skip_css_whitespace_and_comments(bytes, start);

    if would_start_identifier(bytes, index) {
        let (name, name_end) = consume_identifier(input, index);
        if name.eq_ignore_ascii_case("url") && bytes.get(name_end) == Some(&b'(') {
            let Some((span, next_index)) = parse_url_value(input, name_end.saturating_add(1))
            else {
                return (Some(CssImportReference::Ambiguous), None, bytes.len());
            };
            let reference = if span.ambiguous {
                CssImportReference::Ambiguous
            } else {
                CssImportReference::Target(span.value.clone())
            };
            return (
                Some(reference),
                Some(span),
                statement_end(bytes, next_index),
            );
        }
    }

    let Some(&quote @ (b'\'' | b'"')) = bytes.get(index) else {
        return (
            Some(CssImportReference::Ambiguous),
            None,
            statement_end(bytes, index),
        );
    };
    index = index.saturating_add(1);
    let value_start = index;
    let mut ambiguous = false;

    for _ in 0..bytes.len().saturating_add(1) {
        let Some(&byte) = bytes.get(index) else {
            break;
        };
        if byte == b'\\' {
            // Path payload escapes can encode delimiters and are not guessed here
            ambiguous = true;
            index = consume_escape(input, index).1;
            continue;
        }
        if byte == quote {
            let value = input[value_start..index].to_string();
            // Starting at the closing quote keeps the delimiter search byte-exact
            let next_index = statement_end(bytes, index);
            return if ambiguous {
                (Some(CssImportReference::Ambiguous), None, next_index)
            } else {
                (Some(CssImportReference::Target(value)), None, next_index)
            };
        }
        if matches!(byte, b'\n' | b'\r' | b'\x0c') {
            // A broken quoted import must not hide later active tokens on the next line
            return (Some(CssImportReference::Ambiguous), None, index);
        }
        index = index.saturating_add(utf8_char_len(byte));
    }

    (Some(CssImportReference::Ambiguous), None, bytes.len())
}

fn statement_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| *byte == b';')
        .map_or(bytes.len(), |offset| {
            start.saturating_add(offset).saturating_add(1)
        })
}
