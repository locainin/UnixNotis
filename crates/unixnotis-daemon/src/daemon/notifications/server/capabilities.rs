pub(super) fn notification_capabilities(supports_sound: bool) -> Vec<String> {
    // Advertise only semantics preserved by normalization. Notification bodies
    // are intentionally sanitized to display text, so body-markup is unsupported
    let mut caps = vec![
        "actions".to_string(),
        "inline-reply".to_string(),
        "body".to_string(),
        "icon-static".to_string(),
    ];
    if supports_sound {
        caps.push("sound".to_string());
    }
    caps
}

#[cfg(test)]
#[path = "tests/capabilities.rs"]
mod tests;
