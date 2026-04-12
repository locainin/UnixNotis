use std::borrow::Cow;

pub(super) struct UrlValueSpan {
    // Raw url(...) payload after quotes and outer spacing are trimmed away
    pub(super) value: String,
    // Byte range inside the original CSS string where the payload lived
    pub(super) value_start: usize,
    pub(super) value_end: usize,
}

pub(super) fn collect_url_values(css_text: &str) -> Vec<String> {
    // Most callers only need the trimmed payload values, not the byte ranges
    collect_url_spans(css_text)
        .into_iter()
        .map(|span| span.value)
        .collect()
}

pub(super) fn collect_url_spans(css_text: &str) -> Vec<UrlValueSpan> {
    let bytes = css_text.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut in_comment = false;

    // URL scanning stays byte-based so the parser can rewrite exact slices later on
    while index < bytes.len() {
        if in_comment {
            // Comments are skipped inline so commented-out examples never reach later checks
            if index + 1 < bytes.len() && bytes[index] == b'*' && bytes[index + 1] == b'/' {
                in_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }

        if index + 1 < bytes.len() && bytes[index] == b'/' && bytes[index + 1] == b'*' {
            in_comment = true;
            index += 2;
            continue;
        }

        if starts_with_url(bytes, index) {
            // Each match returns the exact payload range so the caller can replace just that text
            let open_index = index + 4;
            let Some((span, next_index)) = parse_url_value(css_text, open_index) else {
                break;
            };
            spans.push(span);
            index = next_index;
            continue;
        }

        index += 1;
    }

    spans
}

pub(super) fn strip_css_comments(input: &str) -> Cow<'_, str> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_comment = false;
    let mut changed = false;

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                chars.next();
                in_comment = false;
            }
            changed = true;
            continue;
        }
        if ch == '/' && matches!(chars.peek(), Some('*')) {
            chars.next();
            in_comment = true;
            changed = true;
            continue;
        }
        output.push(ch);
    }

    if changed {
        Cow::Owned(output)
    } else {
        Cow::Borrowed(input)
    }
}

fn starts_with_url(bytes: &[u8], index: usize) -> bool {
    // URL matching stays ASCII-only so scanning never slices through UTF-8 code points
    index + 4 <= bytes.len()
        && bytes[index].eq_ignore_ascii_case(&b'u')
        && bytes[index + 1].eq_ignore_ascii_case(&b'r')
        && bytes[index + 2].eq_ignore_ascii_case(&b'l')
        && bytes[index + 3] == b'('
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

    let mut value = String::new();
    let mut value_end;
    let mut quote = None::<u8>;
    if matches!(bytes[index], b'\'' | b'"') {
        // Quoted URLs keep the quote out of the stored payload and later rewrite
        quote = Some(bytes[index]);
        index += 1;
    }
    let value_start = index;
    value_end = index;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(open_quote) = quote {
            if byte == open_quote {
                quote = None;
            } else {
                value.push(byte as char);
                value_end = index + 1;
            }
            index += 1;
            continue;
        }

        match byte {
            b')' => {
                // Trimming here keeps later path checks away from harmless outer spacing
                return Some((
                    UrlValueSpan {
                        value: value.trim().to_string(),
                        value_start,
                        value_end,
                    },
                    index + 1,
                ));
            }
            b'\'' | b'"' => {
                // Unquoted url(...) is malformed, but keeping the byte preserves caller visibility
                value.push(byte as char);
                value_end = index + 1;
            }
            _ => {
                value.push(byte as char);
                if !byte.is_ascii_whitespace() || !value.trim().is_empty() {
                    value_end = index + 1;
                }
            }
        }
        index += 1;
    }

    None
}
