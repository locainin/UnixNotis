use super::types::CssDeclaration;

pub(in super::super) fn parse_css_declarations(block: &str) -> Vec<(String, String)> {
    // Older callers only need declaration text, not source offsets
    parse_css_declarations_with_offsets(block)
        .into_iter()
        .map(|declaration| (declaration.name, declaration.value))
        .collect()
}

pub(in super::super) fn parse_css_declarations_with_offsets(block: &str) -> Vec<CssDeclaration> {
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
                // Only top-level semicolons end a declaration
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
