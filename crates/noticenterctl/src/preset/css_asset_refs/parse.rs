use anyhow::{bail, Result};

const MAX_CSS_REFERENCES_PER_FILE: usize = 4_096;

pub(super) struct UrlValueSpan {
    // Raw url(...) payload after quotes and outer spacing are trimmed away
    pub(super) value: String,
    // Byte range inside the original CSS string where the payload lived
    pub(super) value_start: usize,
    pub(super) value_end: usize,
    // Escapes require a full CSS tokenizer, so callers fail closed instead of guessing
    pub(super) ambiguous: bool,
}

#[derive(Debug)]
pub(super) struct CssReference {
    pub(super) value: String,
    pub(super) ambiguous: bool,
}

#[derive(Debug)]
pub enum CssImportReference {
    Target(String),
    Ambiguous,
}

pub(super) fn collect_url_values(css_text: &str) -> Result<Vec<CssReference>> {
    // Most callers only need the trimmed payload values, not the byte ranges
    Ok(collect_url_spans(css_text)?
        .into_iter()
        .map(|span| CssReference {
            value: span.value,
            ambiguous: span.ambiguous,
        })
        .collect())
}

pub(super) fn collect_import_values(css_text: &str) -> Result<Vec<CssImportReference>> {
    collect_import_values_with_url_forms(css_text, false)
}

pub fn collect_import_dependency_values(css_text: &str) -> Result<Vec<CssImportReference>> {
    // Cache discovery needs url(...) imports while asset discovery already sees them as URLs
    collect_import_values_with_url_forms(css_text, true)
}

fn collect_import_values_with_url_forms(
    css_text: &str,
    include_url_forms: bool,
) -> Result<Vec<CssImportReference>> {
    let bytes = css_text.as_bytes();
    let mut imports = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                // Strings may contain example text that looks like an import directive
                index = skip_quoted_value(bytes, index).unwrap_or(bytes.len());
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                // Comments are ignored for the same reason as strings
                index = skip_comment(bytes, index + 2).unwrap_or(bytes.len());
            }
            b'@' if starts_with_import(bytes, index) => {
                let (reference, next_index) =
                    parse_import_value(css_text, index + 7, include_url_forms);
                if let Some(reference) = reference {
                    if imports.len() >= MAX_CSS_REFERENCES_PER_FILE {
                        bail!(
                            "CSS file contains more than {MAX_CSS_REFERENCES_PER_FILE} import references"
                        );
                    }
                    imports.push(reference);
                }
                index = next_index;
            }
            _ => index += 1,
        }
    }

    Ok(imports)
}

pub(super) fn collect_url_spans(css_text: &str) -> Result<Vec<UrlValueSpan>> {
    let bytes = css_text.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;

    // URL scanning stays byte-based so the parser can rewrite exact slices later on
    while index < bytes.len() {
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            // One shared comment skipper keeps URL and import discovery consistent
            index = skip_comment(bytes, index.saturating_add(2)).unwrap_or(bytes.len());
            continue;
        }

        if matches!(bytes[index], b'\'' | b'"') {
            // Property strings may contain documentation text such as "url(example)"
            index = skip_quoted_value(bytes, index).unwrap_or(bytes.len());
            continue;
        }

        if starts_with_url(bytes, index) {
            // Each match returns the exact payload range so the caller can replace just that text
            let open_index = index + 4;
            let (span, next_index) = parse_url_value(css_text, open_index).ok_or_else(|| {
                anyhow::anyhow!("CSS contains an unterminated url(...) reference")
            })?;
            if spans.len() >= MAX_CSS_REFERENCES_PER_FILE {
                bail!("CSS file contains more than {MAX_CSS_REFERENCES_PER_FILE} URL references");
            }
            spans.push(span);
            index = next_index;
            continue;
        }

        index += 1;
    }

    Ok(spans)
}

fn starts_with_url(bytes: &[u8], index: usize) -> bool {
    // URL matching stays ASCII-only so scanning never slices through UTF-8 code points
    let has_token_boundary = index == 0
        || bytes
            .get(index - 1)
            .is_some_and(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_'));
    has_token_boundary
        && index + 4 <= bytes.len()
        && bytes[index].eq_ignore_ascii_case(&b'u')
        && bytes[index + 1].eq_ignore_ascii_case(&b'r')
        && bytes[index + 2].eq_ignore_ascii_case(&b'l')
        && bytes[index + 3] == b'('
}

fn starts_with_import(bytes: &[u8], index: usize) -> bool {
    const KEYWORD: &[u8] = b"@import";
    if index + KEYWORD.len() > bytes.len()
        || !bytes[index..index + KEYWORD.len()].eq_ignore_ascii_case(KEYWORD)
    {
        return false;
    }

    // A name character after the keyword means this is a different at-rule
    bytes
        .get(index + KEYWORD.len())
        .is_none_or(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_'))
}

fn parse_import_value(
    input: &str,
    start: usize,
    include_url_forms: bool,
) -> (Option<CssImportReference>, usize) {
    let bytes = input.as_bytes();
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }

    if starts_with_url(bytes, index) {
        // Asset discovery already sees url(...) while cache discovery requests its target here
        if !include_url_forms {
            return (None, statement_end(bytes, index));
        }
        let Some((span, next_index)) = parse_url_value(input, index + 4) else {
            return (Some(CssImportReference::Ambiguous), bytes.len());
        };
        let reference = if span.ambiguous {
            CssImportReference::Ambiguous
        } else {
            CssImportReference::Target(span.value)
        };
        return (Some(reference), statement_end(bytes, next_index));
    }

    let Some(&quote @ (b'\'' | b'"')) = bytes.get(index) else {
        return (
            Some(CssImportReference::Ambiguous),
            statement_end(bytes, index),
        );
    };
    index += 1;
    let value_start = index;
    let mut escaped = false;

    while let Some(&byte) = bytes.get(index) {
        if byte == b'\\' {
            // CSS escapes can encode separators and schemes, so path checks cannot decode them here
            escaped = true;
            index = index.saturating_add(2);
            continue;
        }
        if byte == quote {
            let value = input[value_start..index].to_string();
            let next_index = statement_end(bytes, index.saturating_add(1));
            if escaped {
                return (Some(CssImportReference::Ambiguous), next_index);
            }
            return (Some(CssImportReference::Target(value)), next_index);
        }
        index += 1;
    }

    (Some(CssImportReference::Ambiguous), bytes.len())
}

fn statement_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| *byte == b';')
        .map_or(bytes.len(), |offset| start + offset + 1)
}

fn skip_quoted_value(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let mut index = start + 1;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'\\' {
            index = index.saturating_add(2);
            continue;
        }
        if byte == quote {
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

fn skip_comment(bytes: &[u8], start: usize) -> Option<usize> {
    bytes[start..]
        .windows(2)
        .position(|window| window == b"*/")
        .map(|offset| start + offset + 2)
}

fn parse_url_value(input: &str, open_index: usize) -> Option<(UrlValueSpan, usize)> {
    let bytes = input.as_bytes();
    let mut index = open_index;

    // Leading whitespace after url( does not matter, so it is skipped before capture starts
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index >= bytes.len() {
        return None;
    }

    let mut ambiguous = false;
    if let quote @ (b'\'' | b'"') = bytes[index] {
        // Quoted values end at their matching unescaped quote
        let value_start = index + 1;
        index = value_start;
        while let Some(&byte) = bytes.get(index) {
            if byte == b'\\' {
                ambiguous = true;
                index = index.saturating_add(2);
                continue;
            }
            if byte == quote {
                let value_end = index;
                index += 1;
                while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                    index += 1;
                }
                if bytes.get(index) != Some(&b')') {
                    return None;
                }
                return Some((
                    UrlValueSpan {
                        value: input[value_start..value_end].to_string(),
                        value_start,
                        value_end,
                        ambiguous,
                    },
                    index + 1,
                ));
            }
            index += 1;
        }
        return None;
    }

    // Unquoted values stop at the first unescaped closing parenthesis
    let raw_start = index;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'\\' {
            ambiguous = true;
            index = index.saturating_add(2);
            continue;
        }
        if byte == b')' {
            let raw = &input[raw_start..index];
            let value = raw.trim();
            let leading_space = raw.len().saturating_sub(raw.trim_start().len());
            let value_start = raw_start.saturating_add(leading_space);
            let value_end = value_start.saturating_add(value.len());
            return Some((
                UrlValueSpan {
                    value: value.to_string(),
                    value_start,
                    value_end,
                    ambiguous,
                },
                index + 1,
            ));
        }
        if matches!(byte, b'\'' | b'"') {
            // Quotes inside an unquoted value require a full CSS tokenizer to interpret
            ambiguous = true;
        }
        index += 1;
    }
    None
}

#[cfg(test)]
#[path = "tests/parse.rs"]
mod tests;
