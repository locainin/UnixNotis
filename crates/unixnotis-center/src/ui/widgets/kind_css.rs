//! Widget kind CSS class helpers

pub(super) fn widget_kind_css_class(prefix: &str, kind: &str) -> Option<String> {
    let token = css_safe_kind_token(kind)?;
    Some(format!("{prefix}{token}"))
}

fn css_safe_kind_token(kind: &str) -> Option<String> {
    // GTK class names need plain tokens, so punctuation becomes a stable separator
    let mut out = String::new();
    let mut last_dash = false;

    for ch in kind.chars() {
        let ch = match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => '-',
        };

        if ch == '-' {
            // One dash is enough to show a boundary
            if last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }

        out.push(ch);
    }

    let token = out.trim_matches('-');
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

#[cfg(test)]
#[path = "tests/kind_css.rs"]
mod tests;
