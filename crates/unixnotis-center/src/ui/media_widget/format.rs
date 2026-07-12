use std::collections::BTreeMap;

use crate::media::MediaInfo;
use unixnotis_core::{MediaConfig, MediaPositionFormat, MediaTitleFallback};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MediaDisplayConfig {
    pub(super) show_source: bool,
    pub(super) show_source_when_single_player: bool,
    pub(super) show_position: bool,
    pub(super) show_position_when_single_player: bool,
    pub(super) show_title: bool,
    pub(super) show_artist: bool,
    pub(super) title_fallback: MediaTitleFallback,
    pub(super) position_format: MediaPositionFormat,
    pub(super) source_aliases: BTreeMap<String, String>,
}

impl MediaDisplayConfig {
    pub(super) fn from_config(config: &MediaConfig) -> Self {
        Self {
            show_source: config.show_source,
            show_source_when_single_player: config.show_source_when_single_player,
            show_position: config.show_position,
            show_position_when_single_player: config.show_position_when_single_player,
            show_title: config.show_title,
            show_artist: config.show_artist,
            title_fallback: config.title_fallback,
            position_format: config.position_format,
            source_aliases: config.source_aliases.clone(),
        }
    }
}

pub(super) fn title_text_for(info: &MediaInfo, display: &MediaDisplayConfig) -> Option<String> {
    if !display.show_title {
        return None;
    }
    if !info.title.trim().is_empty() {
        return Some(info.title.clone());
    }
    match display.title_fallback {
        MediaTitleFallback::Identity => Some(resolve_source_label(info, &display.source_aliases)),
        MediaTitleFallback::Artist => {
            if info.artist.trim().is_empty() {
                None
            } else {
                Some(info.artist.clone())
            }
        }
        MediaTitleFallback::Empty => None,
    }
}

pub(super) fn source_text_for(
    info: &MediaInfo,
    total: usize,
    display: &MediaDisplayConfig,
) -> Option<String> {
    if !display.show_source {
        return None;
    }
    if total <= 1 && !display.show_source_when_single_player {
        return None;
    }
    Some(resolve_source_label(info, &display.source_aliases))
}

pub(super) fn position_text_for(
    current: usize,
    total: usize,
    display: &MediaDisplayConfig,
) -> Option<String> {
    if !display.show_position {
        return None;
    }
    if total <= 1 && !display.show_position_when_single_player {
        return None;
    }
    match display.position_format {
        MediaPositionFormat::Fraction => Some(format!("{current}/{total}")),
        MediaPositionFormat::Current => Some(current.to_string()),
    }
}

fn resolve_source_label(info: &MediaInfo, aliases: &BTreeMap<String, String>) -> String {
    // The common config leaves aliases empty, so skip normalization work on that fast path
    if aliases.is_empty() {
        return default_source_label(info);
    }

    let identity = info.identity.trim();
    let bus_name = info.bus_name.trim().to_lowercase();
    let identity_lower = identity.to_lowercase();

    // Prefer the longest token so specific aliases win over broad tokens
    let mut best: Option<(&str, usize)> = None;
    for (token, label) in aliases {
        if !identity_lower.contains(token) && !bus_name.contains(token) {
            continue;
        }
        let score = token.len();
        if best.is_none_or(|(_, current)| score > current) {
            best = Some((label.as_str(), score));
        }
    }
    best.map(|(label, _)| label.to_string())
        .unwrap_or_else(|| default_source_label(info))
}

fn default_source_label(info: &MediaInfo) -> String {
    if !info.identity.trim().is_empty() {
        return info.identity.trim().to_string();
    }

    // Bus names are noisier, so only the tail gets shown when identity is missing
    info.bus_name
        .rsplit('.')
        .next()
        .filter(|segment| !segment.trim().is_empty())
        .unwrap_or("Unknown Player")
        .to_string()
}

#[cfg(test)]
#[path = "tests/format.rs"]
mod tests;
