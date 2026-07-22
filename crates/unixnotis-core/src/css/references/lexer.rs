//! CSS identifier, escape, comment, and whitespace primitives

pub(super) fn identifier_matches(input: &str, start: usize, expected: &str) -> (bool, usize) {
    let bytes = input.as_bytes();
    let expected = expected.as_bytes();
    let mut matched = true;
    let mut decoded_len = 0usize;
    let mut index = start;

    // Security scanners recognize a small fixed vocabulary without allocating every identifier
    for _ in 0..bytes.len().saturating_add(1) {
        let Some(&byte) = bytes.get(index) else {
            break;
        };
        let (decoded, next_index) = if is_name_byte(byte) {
            let decoded = if byte.is_ascii() {
                char::from(byte)
            } else {
                input[index..].chars().next().unwrap_or('\u{FFFD}')
            };
            (decoded, index.saturating_add(decoded.len_utf8()))
        } else if valid_escape(bytes, index) {
            consume_escape(input, index)
        } else {
            break;
        };

        if next_index <= index {
            break;
        }
        matched &= expected
            .get(decoded_len)
            .is_some_and(|expected| decoded.eq_ignore_ascii_case(&char::from(*expected)));
        decoded_len = decoded_len.saturating_add(1);
        index = next_index;
    }

    (matched && decoded_len == expected.len(), index)
}

pub(super) fn consume_escape(input: &str, slash_index: usize) -> (char, usize) {
    let bytes = input.as_bytes();
    let mut index = slash_index.saturating_add(1);
    let Some(&first) = bytes.get(index) else {
        return ('\u{FFFD}', index);
    };

    if first.is_ascii_hexdigit() {
        let digit_start = index;
        let mut digits = 0usize;
        while digits < 6 && bytes.get(index).is_some_and(u8::is_ascii_hexdigit) {
            digits = digits.saturating_add(1);
            index = index.saturating_add(1);
        }
        let scalar = u32::from_str_radix(&input[digit_start..index], 16).unwrap_or(0);
        index = consume_escape_terminator(bytes, index);
        // CSS replaces invalid scalar values with the replacement character
        return (
            if scalar == 0 {
                '\u{FFFD}'
            } else {
                char::from_u32(scalar).unwrap_or('\u{FFFD}')
            },
            index,
        );
    }

    if first == b'\r' && bytes.get(index.saturating_add(1)) == Some(&b'\n') {
        return ('\u{FFFD}', index.saturating_add(2));
    }
    if matches!(first, b'\n' | b'\r' | b'\x0c') {
        return ('\u{FFFD}', index.saturating_add(1));
    }

    let ch = input[index..].chars().next().unwrap_or('\u{FFFD}');
    index = index.saturating_add(ch.len_utf8());
    (ch, index)
}

pub(super) fn skip_css_whitespace_and_comments(bytes: &[u8], mut index: usize) -> usize {
    // The finite budget also protects callers if a future skipper stops advancing
    for _ in 0..bytes.len().saturating_add(1) {
        index = skip_css_whitespace(bytes, index);
        if !starts_comment(bytes, index) {
            return index;
        }
        index = skip_comment(bytes, index).unwrap_or(bytes.len());
    }
    index
}

pub(super) fn skip_css_whitespace(bytes: &[u8], mut index: usize) -> usize {
    for _ in 0..bytes.len().saturating_add(1) {
        if !bytes
            .get(index)
            .is_some_and(|byte| is_css_whitespace(*byte))
        {
            break;
        }
        index = index.saturating_add(1);
    }
    index
}

pub(super) fn trim_css_whitespace_range(
    bytes: &[u8],
    mut start: usize,
    mut end: usize,
) -> (usize, usize) {
    while start < end
        && bytes
            .get(start)
            .is_some_and(|byte| is_css_whitespace(*byte))
    {
        start = start.saturating_add(1);
    }
    while end > start
        && end
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .is_some_and(|byte| is_css_whitespace(*byte))
    {
        end = end.saturating_sub(1);
    }
    (start, end)
}

pub(super) fn skip_quoted_value(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let quote = *bytes.get(start)?;
    let mut index = start.saturating_add(1);
    for _ in 0..bytes.len().saturating_add(1) {
        let Some(&byte) = bytes.get(index) else {
            break;
        };
        if byte == b'\\' {
            let next_index = consume_escape(input, index).1;
            if next_index <= index {
                return None;
            }
            index = next_index;
            continue;
        }
        if byte == quote {
            return Some(index.saturating_add(1));
        }
        if matches!(byte, b'\n' | b'\r' | b'\x0c') {
            // A raw CSS newline ends a bad string token and scanning resumes there
            return Some(index);
        }
        index = index.saturating_add(utf8_char_len(byte));
    }
    None
}

pub(super) fn skip_comment(bytes: &[u8], start: usize) -> Option<usize> {
    bytes[start..]
        .windows(2)
        .position(|window| window == b"*/")
        .map(|offset| start.saturating_add(offset).saturating_add(2))
}

pub(super) fn starts_comment(bytes: &[u8], index: usize) -> bool {
    bytes.get(index..index.saturating_add(2)) == Some(b"/*")
}

pub(super) fn would_start_identifier(bytes: &[u8], index: usize) -> bool {
    let Some(&byte) = bytes.get(index) else {
        return false;
    };
    is_name_start_byte(byte) || byte == b'-' || (byte == b'\\' && valid_escape(bytes, index))
}

pub(super) fn valid_escape(bytes: &[u8], index: usize) -> bool {
    bytes.get(index) == Some(&b'\\')
        && matches!(bytes.get(index.saturating_add(1)), Some(next) if !matches!(next, b'\n' | b'\r' | b'\x0c'))
}

const fn is_name_start_byte(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || !byte.is_ascii()
}

const fn is_name_byte(byte: u8) -> bool {
    is_name_start_byte(byte) || byte.is_ascii_digit() || byte == b'-'
}

const fn is_css_whitespace(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

pub(super) const fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

fn consume_escape_terminator(bytes: &[u8], index: usize) -> usize {
    match bytes.get(index) {
        Some(b'\r') if bytes.get(index.saturating_add(1)) == Some(&b'\n') => {
            index.saturating_add(2)
        }
        Some(byte) if is_css_whitespace(*byte) => index.saturating_add(1),
        _ => index,
    }
}
