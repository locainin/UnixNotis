//! Token splitting helpers for geometry length parsing

pub(super) fn consume_balanced_group(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut cursor = start;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;

    while cursor < bytes.len() {
        let ch = input[cursor..].chars().next()?;
        cursor += ch.len_utf8();

        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => {
                // Group ends only when both paren and bracket nesting are back at zero
                paren_depth = paren_depth.saturating_sub(1);
                if paren_depth == 0 && bracket_depth == 0 {
                    return Some(cursor);
                }
            }
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
    }

    None
}

pub(super) fn split_css_value_tokens(value: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = None::<usize>;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;

    for (index, ch) in value.char_indices() {
        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            if start.is_none() {
                start = Some(index);
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                if start.is_none() {
                    start = Some(index);
                }
                in_string = Some(ch);
            }
            '(' => {
                if start.is_none() {
                    start = Some(index);
                }
                paren_depth = paren_depth.saturating_add(1);
            }
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => {
                if start.is_none() {
                    start = Some(index);
                }
                bracket_depth = bracket_depth.saturating_add(1);
            }
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if ch.is_whitespace() && paren_depth == 0 && bracket_depth == 0 => {
                if let Some(token_start) = start.take() {
                    // Top-level whitespace is the only real shorthand separator
                    tokens.push(value[token_start..index].trim());
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(index);
                }
            }
        }
    }

    if let Some(token_start) = start {
        tokens.push(value[token_start..].trim());
    }

    tokens
        .into_iter()
        .filter(|token| !token.is_empty())
        .collect()
}

pub(super) fn split_top_level_once(input: &str, separator: char) -> (&str, Option<&str>) {
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;

    for (index, ch) in input.char_indices() {
        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if ch == separator && paren_depth == 0 && bracket_depth == 0 => {
                // Only the first top-level separator matters for var() fallback splitting
                let right = index + ch.len_utf8();
                return (&input[..index], Some(&input[right..]));
            }
            _ => {}
        }
    }

    (input, None)
}

pub(super) fn split_top_level_list(input: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;

    for (index, ch) in input.char_indices() {
        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if ch == separator && paren_depth == 0 && bracket_depth == 0 => {
                // Top-level commas split function arguments without breaking nested math
                let part = input[start..index].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }

    parts
}
