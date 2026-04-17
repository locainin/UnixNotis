use std::collections::{BTreeMap, HashSet};

use tracing::warn;

use super::{
    Config, DEFAULT_MEDIA_ART_SIZE_PX, DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX, MAX_CARD_HEIGHT,
    MAX_MEDIA_ART_SIZE, MAX_MEDIA_TEXT_WIDTH_FLOOR, MAX_MEDIA_TITLE_CHAR_LIMIT, MAX_SPACING,
    MIN_MEDIA_TEXT_WIDTH_FLOOR, MIN_MEDIA_TITLE_CHAR_LIMIT,
};

pub(super) fn sanitize_media_config(config: &mut Config) {
    // Normalize token lists once so the hot path does not keep trimming and folding case
    config.media.allowlist = normalize_media_tokens(&config.media.allowlist);
    config.media.denylist = normalize_media_tokens(&config.media.denylist);
    config.media.browser_tokens = normalize_media_tokens(&config.media.browser_tokens);
    config.media.source_aliases = normalize_media_aliases(&config.media.source_aliases);

    // Media text and geometry should stay readable without letting configs explode
    config.media.title_char_limit = config
        .media
        .title_char_limit
        .clamp(MIN_MEDIA_TITLE_CHAR_LIMIT, MAX_MEDIA_TITLE_CHAR_LIMIT);
    if config.media.art_size_px <= 0 {
        // Pull the scalar default directly instead of building a full MediaConfig
        config.media.art_size_px = DEFAULT_MEDIA_ART_SIZE_PX;
    }
    config.media.art_size_px = config.media.art_size_px.clamp(1, MAX_MEDIA_ART_SIZE);
    if config.media.text_width_floor_px <= 0 {
        // Same rule here so sanitize stays cheap and explicit
        config.media.text_width_floor_px = DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX;
    }
    config.media.text_width_floor_px = config
        .media
        .text_width_floor_px
        .clamp(MIN_MEDIA_TEXT_WIDTH_FLOOR, MAX_MEDIA_TEXT_WIDTH_FLOOR);
    config.media.content_spacing_px = config.media.content_spacing_px.clamp(0, MAX_SPACING);
    config.media.control_spacing_px = config.media.control_spacing_px.clamp(0, MAX_SPACING);
    config.media.navigation_spacing_px = config.media.navigation_spacing_px.clamp(0, MAX_SPACING);
    if let Some(card_height_px) = config.media.card_height_px {
        if card_height_px <= 0 {
            config.media.card_height_px = None;
        } else {
            config.media.card_height_px = Some(card_height_px.clamp(1, MAX_CARD_HEIGHT));
        }
    }
}

fn normalize_media_tokens(tokens: &[String]) -> Vec<String> {
    let mut normalized = Vec::with_capacity(tokens.len());
    let mut seen = HashSet::with_capacity(tokens.len());

    for token in tokens {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            // Empty tokens cannot match anything useful later
            warn!(raw_token = token, "discarding empty media token");
            continue;
        }

        let normalized_token = trimmed.to_lowercase();
        if !seen.insert(normalized_token.clone()) {
            // Duplicate tokens only repeat the same later comparison
            warn!(token = normalized_token, "discarding duplicate media token");
            continue;
        }

        normalized.push(normalized_token);
    }

    normalized
}

fn normalize_media_aliases(aliases: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut normalized = BTreeMap::new();
    let mut original_keys: BTreeMap<String, String> = BTreeMap::new();

    for (token, label) in aliases {
        let trimmed_token = token.trim();
        let trimmed_label = label.trim();

        if trimmed_token.is_empty() {
            // Empty alias keys never match a player name or identity
            warn!(
                raw_key = token,
                raw_label = label,
                "discarding media alias with empty key"
            );
            continue;
        }
        if trimmed_label.is_empty() {
            // Empty labels render like missing metadata and only confuse the result
            warn!(
                raw_key = token,
                raw_label = label,
                "discarding media alias with empty label"
            );
            continue;
        }

        let normalized_token = trimmed_token.to_lowercase();
        if let Some(existing_key) = original_keys.get(&normalized_token) {
            // Lowercasing can collapse two user keys into the same runtime key
            warn!(
                normalized_key = normalized_token,
                kept_key = existing_key.as_str(),
                dropped_key = token.as_str(),
                "discarding colliding media alias after normalization"
            );
            continue;
        }

        original_keys.insert(normalized_token.clone(), token.clone());
        normalized.insert(normalized_token, trimmed_label.to_string());
    }

    normalized
}
