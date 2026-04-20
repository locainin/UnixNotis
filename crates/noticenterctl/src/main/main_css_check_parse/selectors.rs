pub(in super::super) fn normalize_selector(selector: &str) -> String {
    // Whitespace-only changes should not make selectors look different
    selector
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub(in super::super) fn split_selectors(selector: &str) -> Vec<String> {
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
                // Only top-level commas start a new selector
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

pub(in super::super) fn should_recurse_at_rule(selector: &str) -> bool {
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
