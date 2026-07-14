// Selector checks stay separate so the width parser only deals with declarations
fn rightmost_selector_segment(selector: &str) -> &str {
    // Splitting from the right avoids hand-maintained byte offsets for UTF-8 selectors
    selector
        .rsplit(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '>' | '+' | '~'))
        .find(|segment| !segment.is_empty())
        .unwrap_or(selector)
        .trim()
}

pub(super) fn simple_class_selector(selector: &str) -> Option<&str> {
    let trimmed = selector.trim();
    if !trimmed.starts_with('.') {
        // Element names and IDs are outside the small class-based model used here
        return None;
    }
    if trimmed.contains(' ')
        || trimmed.contains('>')
        || trimmed.contains('+')
        || trimmed.contains('~')
        || trimmed.contains(':')
        || trimmed.contains('[')
        || trimmed.contains('#')
        || trimmed.contains(',')
    {
        // Descendant and pseudo selectors are skipped to keep matching conservative
        return None;
    }
    if trimmed.matches('.').count() != 1 {
        // Compound class chains are ambiguous for this lightweight model
        return None;
    }
    Some(trimmed)
}

pub(super) fn complex_target_class(selector: &str) -> Option<String> {
    let target = rightmost_selector_segment(selector);
    target.split('.').skip(1).find_map(|fragment| {
        let class_name: String = fragment
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
            .collect();
        // The first UnixNotis class in a compound selector is the structural width owner
        class_name
            .starts_with("unixnotis-")
            .then(|| format!(".{class_name}"))
    })
}

pub(super) fn is_nonexpanding_boundary_reset(
    selector: &str,
    properties: &[(String, String)],
) -> bool {
    let boundary_state = selector.ends_with(":first-child") || selector.ends_with(":last-child");
    boundary_state
        && !properties.is_empty()
        && properties.iter().all(|(name, value)| {
            matches!(name.as_str(), "margin-left" | "margin-right")
                && matches!(value.trim(), "0" | "0px")
        })
}
