//! Shared CSS class hooks for widget state

pub mod shared_state {
    // These names are short on purpose because they show up in many theme selectors
    pub const ACTIVE: &str = "active";
    pub const CRITICAL: &str = "critical";
    pub const EMPTY: &str = "empty";
    pub const PLAYING: &str = "playing";
    pub const STACKED: &str = "stacked";
}

pub mod panel_card {
    // Panel-only hooks stay namespaced so they do not collide with popup or media classes
    pub const HAS_ACTIONS: &str = "unixnotis-panel-card-has-actions";
    pub const HAS_BODY: &str = "unixnotis-panel-card-has-body";
    pub const HAS_SUMMARY: &str = "unixnotis-panel-card-has-summary";
    pub const NO_ACTIONS: &str = "unixnotis-panel-card-no-actions";
}

pub mod popup_card {
    // Popup hooks track content shape so themes do not have to inspect widget text
    pub const HAS_ACTIONS: &str = "unixnotis-popup-card-has-actions";
    pub const HAS_BODY: &str = "unixnotis-popup-card-has-body";
    pub const HAS_ICON: &str = "unixnotis-popup-card-has-icon";
    pub const HAS_SUMMARY: &str = "unixnotis-popup-card-has-summary";
    pub const NO_ICON: &str = "unixnotis-popup-card-no-icon";
}

pub mod media_card {
    // Media hooks expose playback and artwork state as stable styling points
    pub const EMPTY_ARTIST: &str = "unixnotis-media-card-empty-artist";
    pub const HAS_ART: &str = "unixnotis-media-card-has-art";
    pub const HAS_ARTIST: &str = "unixnotis-media-card-has-artist";
    pub const MULTI_PLAYER: &str = "unixnotis-media-card-multi-player";
    pub const NO_ART: &str = "unixnotis-media-card-no-art";
    pub const PAUSED: &str = "unixnotis-media-card-paused";
    pub const PLAYING: &str = "unixnotis-media-card-playing";
    pub const SINGLE_PLAYER: &str = "unixnotis-media-card-single-player";
    pub const STOPPED: &str = "unixnotis-media-card-stopped";
}

#[cfg(test)]
mod tests {
    use super::{media_card, panel_card, popup_card, shared_state};
    use std::collections::HashSet;

    #[test]
    fn hook_names_stay_unique() {
        let names = [
            shared_state::ACTIVE,
            shared_state::CRITICAL,
            shared_state::EMPTY,
            shared_state::PLAYING,
            shared_state::STACKED,
            panel_card::HAS_ACTIONS,
            panel_card::HAS_BODY,
            panel_card::HAS_SUMMARY,
            panel_card::NO_ACTIONS,
            popup_card::HAS_ACTIONS,
            popup_card::HAS_BODY,
            popup_card::HAS_ICON,
            popup_card::HAS_SUMMARY,
            popup_card::NO_ICON,
            media_card::EMPTY_ARTIST,
            media_card::HAS_ART,
            media_card::HAS_ARTIST,
            media_card::MULTI_PLAYER,
            media_card::NO_ART,
            media_card::PAUSED,
            media_card::PLAYING,
            media_card::SINGLE_PLAYER,
            media_card::STOPPED,
        ];
        let unique = names.iter().copied().collect::<HashSet<_>>();
        assert_eq!(unique.len(), names.len());
    }
}
