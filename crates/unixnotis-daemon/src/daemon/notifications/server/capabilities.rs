pub(super) fn notification_capabilities(supports_sound: bool) -> Vec<String> {
    // Capabilities are static except for optional sound support
    let mut caps = vec![
        "actions".to_string(),
        "body".to_string(),
        "body-markup".to_string(),
        "icon-static".to_string(),
    ];
    if supports_sound {
        caps.push("sound".to_string());
    }
    caps
}

#[cfg(test)]
#[path = "../tests/capabilities.rs"]
mod tests;
