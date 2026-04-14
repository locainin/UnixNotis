//! Small CSS scanner helpers for css-check

pub(super) struct CssBlock {
    // Selector text before normalization
    pub(super) selector: String,
    // Raw block body without the outer braces
    pub(super) block: String,
    // Cursor position for the next scan step
    pub(super) next: usize,
    // Absolute offsets let lint map warnings back to line and column
    pub(super) selector_start: usize,
    pub(super) block_start: usize,
}

pub(super) struct CssDeclaration {
    pub(super) name: String,
    pub(super) value: String,
    // Declaration offsets are relative to the block slice
    pub(super) start: usize,
}

pub(super) fn strip_css_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_comment = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            // Stay in comment mode until the closing marker is found
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                output.push(' ');
                output.push(' ');
                chars.next();
                in_comment = false;
                continue;
            }
            if ch == '\n' || ch == '\r' {
                // Newlines need to stay in place so line numbers still match
                output.push(ch);
            } else {
                output.push(' ');
            }
            continue;
        }
        if ch == '/' && matches!(chars.peek(), Some('*')) {
            // Replace the opening marker too so offsets stay aligned with the source
            output.push(' ');
            output.push(' ');
            chars.next();
            in_comment = true;
            continue;
        }
        output.push(ch);
    }
    output
}

pub(super) fn next_css_block(bytes: &[u8], start: usize) -> Option<(String, String, usize)> {
    // Older callers only need the text, so this stays as a small adapter
    next_css_block_with_offsets(bytes, start).map(|block| (block.selector, block.block, block.next))
}

pub(super) fn next_css_block_with_offsets(bytes: &[u8], start: usize) -> Option<CssBlock> {
    // This scanner only needs to find balanced blocks
    let mut selector_start = start;
    while selector_start < bytes.len() && bytes[selector_start].is_ascii_whitespace() {
        selector_start += 1;
    }

    let mut index = selector_start;
    let mut in_string: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote) = in_string {
            if byte == quote {
                in_string = None;
            }
            index += 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            in_string = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'{' {
            let selector = String::from_utf8_lossy(&bytes[selector_start..index]).to_string();
            let mut depth = 1usize;
            index += 1;
            let block_start = index;
            while index < bytes.len() {
                let byte = bytes[index];
                if let Some(quote) = in_string {
                    if byte == quote {
                        in_string = None;
                    }
                    index += 1;
                    continue;
                }
                if byte == b'"' || byte == b'\'' {
                    in_string = Some(byte);
                    index += 1;
                    continue;
                }
                if byte == b'{' {
                    depth += 1;
                } else if byte == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        let block = String::from_utf8_lossy(&bytes[block_start..index]).to_string();
                        return Some(CssBlock {
                            selector,
                            block,
                            next: index + 1,
                            selector_start,
                            block_start,
                        });
                    }
                }
                index += 1;
            }
            break;
        }
        index += 1;
    }
    None
}

pub(super) fn normalize_selector(selector: &str) -> String {
    // Whitespace-only changes should not make selectors look different
    selector
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub(super) fn split_selectors(selector: &str) -> Vec<String> {
    // Split grouped selectors, but not commas inside nested parts
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string: Option<char> = None;
    let mut chars = selector.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(quote) = in_string {
            if ch == '\\' {
                current.push(ch);
                if let Some(next_char) = chars.next() {
                    current.push(next_char);
                }
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            current.push(ch);
            continue;
        }

        match ch {
            '"' | '\'' => {
                in_string = Some(ch);
                current.push(ch);
            }
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                current.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if paren_depth == 0 && bracket_depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }

    parts
}

pub(super) fn should_recurse_at_rule(selector: &str) -> bool {
    // Only recurse into at-rules that can hold nested selector blocks
    let name = selector
        .trim_start_matches('@')
        .split_whitespace()
        .next()
        .unwrap_or("");
    matches!(
        name,
        "media" | "supports" | "layer" | "container" | "document"
    )
}

pub(super) fn parse_css_declarations(block: &str) -> Vec<(String, String)> {
    // Older callers only need declaration text, not source offsets
    parse_css_declarations_with_offsets(block)
        .into_iter()
        .map(|declaration| (declaration.name, declaration.value))
        .collect()
}

pub(super) fn parse_css_declarations_with_offsets(block: &str) -> Vec<CssDeclaration> {
    // Keep ';' inside quoted text and function args
    let mut declarations = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for (index, ch) in block.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ';' if paren_depth == 0 && bracket_depth == 0 => {
                push_declaration(&mut declarations, &block[start..index], start);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    push_declaration(&mut declarations, &block[start..], start);
    declarations
}

fn push_declaration(declarations: &mut Vec<CssDeclaration>, raw: &str, raw_start: usize) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    // Use the first top-level ':' only
    let Some(colon_index) = find_top_level_colon(trimmed) else {
        return;
    };

    let name = trimmed[..colon_index].trim();
    let value = trimmed[colon_index + 1..].trim();
    if name.is_empty() || value.is_empty() {
        return;
    }

    // Leading space is kept in the offset so later line math points at the property name
    let leading_trim = raw.find(trimmed).unwrap_or(0);
    declarations.push(CssDeclaration {
        name: name.to_string(),
        value: value.to_string(),
        start: raw_start + leading_trim,
    });
}

fn find_top_level_colon(input: &str) -> Option<usize> {
    // Ignore ':' inside quotes and nested groups
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for (index, ch) in input.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ':' if paren_depth == 0 && bracket_depth == 0 => return Some(index),
            _ => {}
        }
    }

    None
}
