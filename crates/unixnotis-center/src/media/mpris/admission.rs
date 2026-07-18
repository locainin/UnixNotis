//! Player allowlist, denylist, and browser-name admission

use unixnotis_core::MediaConfig;

pub(in crate::media) fn is_allowed_player(name: &str, config: &MediaConfig) -> bool {
    let lower = name.to_lowercase();
    if config.denylist.iter().any(|entry| lower.contains(entry)) {
        return false;
    }

    if !config.allowlist.is_empty() {
        return config.allowlist.iter().any(|entry| lower.contains(entry));
    }

    if !config.include_browsers && is_browser_name(&lower, &config.browser_tokens) {
        return false;
    }

    true
}

fn is_browser_name(lower: &str, browser_tokens: &[String]) -> bool {
    // Browser tokens match whole segments so short defaults do not overfire
    browser_tokens
        .iter()
        .any(|token| crate::media::policy::token_matches_segment(lower, token))
}
