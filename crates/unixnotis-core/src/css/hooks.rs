//! Shared CSS class hooks for widget state

pub mod shared_state {
    // These names are short on purpose because they show up in many theme selectors
    pub const ACTIVE: &str = "active";
    pub const CRITICAL: &str = "critical";
    pub const EMPTY: &str = "empty";
    pub const PLAYING: &str = "playing";
    pub const STACKED: &str = "stacked";
}

pub mod panel_action {
    // Panel action hooks expose both shared structure and per-button role
    pub const ROW: &str = "unixnotis-panel-actions";
    pub const GROUP: &str = "unixnotis-panel-action-group";
    pub const ROOT: &str = "unixnotis-panel-action";
    pub const CONTENT: &str = "unixnotis-panel-action-content";
    pub const GLYPH: &str = "unixnotis-panel-action-glyph";
    pub const LABEL: &str = "unixnotis-panel-action-label";
    pub const FOCUS: &str = "unixnotis-panel-action-focus";
    pub const PRIMARY: &str = "unixnotis-panel-action-primary";
    pub const MUTED: &str = "unixnotis-panel-action-muted";
    pub const SEARCH: &str = "unixnotis-panel-action-search";
    pub const CLOSE: &str = "unixnotis-panel-action-close";
    pub const WITH_ICON: &str = "unixnotis-panel-action-with-icon";
    pub const ICON_ONLY: &str = "unixnotis-panel-action-icon";
}

pub mod panel_shell {
    // Panel shell hooks keep the split panel files on one stable class contract
    pub const WINDOW: &str = "unixnotis-panel-window";
    pub const ROOT: &str = "unixnotis-panel";
    pub const HEADER: &str = "unixnotis-panel-header";
    pub const HEADER_TOP: &str = "unixnotis-panel-header-top";
    pub const TITLE_STACK: &str = "unixnotis-panel-title-stack";
    pub const TITLE_ROW: &str = "unixnotis-panel-title-row";
    pub const TITLE: &str = "unixnotis-panel-title";
    pub const COUNT: &str = "unixnotis-panel-count";
    pub const SEARCH: &str = "unixnotis-panel-search";
    pub const SEARCH_REVEALER: &str = "unixnotis-panel-search-revealer";
    pub const MEDIA_CONTAINER: &str = "unixnotis-media-container";
    pub const QUICK_CONTROLS: &str = "unixnotis-quick-controls";
    pub const WIDGET_STACK: &str = "unixnotis-widget-stack";
    pub const WIDGET_REVEALER: &str = "unixnotis-widget-revealer";
    pub const TOGGLE_SECTION: &str = "unixnotis-toggle-section";
    pub const STAT_SECTION: &str = "unixnotis-stat-section";
    pub const CARD_SECTION: &str = "unixnotis-card-section";
}

pub mod panel_card {
    // Panel-only hooks stay namespaced so they do not collide with popup or media classes
    pub const HAS_ACTIONS: &str = "unixnotis-panel-card-has-actions";
    pub const HAS_BODY: &str = "unixnotis-panel-card-has-body";
    pub const HAS_SUMMARY: &str = "unixnotis-panel-card-has-summary";
    pub const NO_ACTIONS: &str = "unixnotis-panel-card-no-actions";
}

pub mod toggle_card {
    // Toggle hooks expose both shell structure and content state
    pub const GRID: &str = "unixnotis-toggle-grid";
    pub const ROOT: &str = "unixnotis-toggle";
    pub const CONTENT: &str = "unixnotis-toggle-content";
    pub const ICON: &str = "unixnotis-toggle-icon";
    pub const LABEL: &str = "unixnotis-toggle-label";
    pub const HAS_ICON: &str = "unixnotis-toggle-has-icon";
    pub const NO_ICON: &str = "unixnotis-toggle-no-icon";
}

pub mod stat_card {
    // Stat hooks separate structure, source type, and icon state
    pub const GRID: &str = "unixnotis-stat-grid";
    pub const ROOT: &str = "unixnotis-stat-card";
    pub const HEADER: &str = "unixnotis-stat-header";
    pub const ICON: &str = "unixnotis-stat-icon";
    pub const TITLE: &str = "unixnotis-stat-title";
    pub const VALUE: &str = "unixnotis-stat-value";
    pub const BUILTIN: &str = "unixnotis-stat-card-builtin";
    pub const PLUGIN: &str = "unixnotis-stat-card-plugin";
    pub const HAS_ICON: &str = "unixnotis-stat-card-has-icon";
    pub const NO_ICON: &str = "unixnotis-stat-card-no-icon";
}

pub mod info_card {
    // Info card hooks cover both base structure and card subtype
    pub const GRID: &str = "unixnotis-card-grid";
    pub const ROOT: &str = "unixnotis-info-card";
    pub const HEADER: &str = "unixnotis-info-header";
    pub const ICON: &str = "unixnotis-info-icon";
    pub const TITLE: &str = "unixnotis-info-title";
    pub const BODY: &str = "unixnotis-info-body";
    pub const CALENDAR_WIDGET: &str = "unixnotis-calendar";
    pub const CALENDAR: &str = "unixnotis-info-card-calendar";
    pub const WEATHER: &str = "unixnotis-info-card-weather";
    pub const MONO: &str = "unixnotis-info-card-mono";
    pub const HAS_ICON: &str = "unixnotis-info-card-has-icon";
    pub const NO_ICON: &str = "unixnotis-info-card-no-icon";
}

pub mod popup_card {
    // Popup hooks track content shape so themes do not have to inspect widget text
    pub const HAS_ACTIONS: &str = "unixnotis-popup-card-has-actions";
    pub const HAS_BODY: &str = "unixnotis-popup-card-has-body";
    pub const HAS_ICON: &str = "unixnotis-popup-card-has-icon";
    pub const HAS_SUMMARY: &str = "unixnotis-popup-card-has-summary";
    pub const NO_ICON: &str = "unixnotis-popup-card-no-icon";
}

pub mod group_row {
    // Group hooks expose both row structure and grouped state
    pub const ROOT: &str = "unixnotis-group";
    pub const CONTAINER: &str = "unixnotis-group-row";
    pub const HEADER: &str = "unixnotis-group-header";
    pub const ICON: &str = "unixnotis-group-icon";
    pub const TITLE: &str = "unixnotis-group-title";
    pub const COUNT: &str = "unixnotis-group-count";
    pub const CHEVRON: &str = "unixnotis-group-chevron";
    pub const COLLAPSED: &str = "unixnotis-group-row-collapsed";
    pub const EXPANDED: &str = "unixnotis-group-row-expanded";
    pub const HAS_ICON: &str = "unixnotis-group-row-has-icon";
    pub const NO_ICON: &str = "unixnotis-group-row-no-icon";
}

pub mod empty_row {
    // Empty list hooks keep placeholder styling stable across list rewrites
    pub const ROOT: &str = "unixnotis-empty";
    pub const LABEL: &str = "unixnotis-empty-label";
}

pub mod ghost_row {
    // Ghost hooks expose stacked placeholder rows used during grouped collapse states
    pub const ROOT: &str = "unixnotis-stack-ghost";
    pub const DEPTH_PREFIX: &str = "unixnotis-stack-ghost-";
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
    use super::{
        empty_row, ghost_row, group_row, info_card, media_card, panel_action, panel_card,
        panel_shell, popup_card, shared_state, stat_card, toggle_card,
    };
    use std::collections::HashSet;

    #[test]
    fn hook_names_stay_unique() {
        let names = [
            shared_state::ACTIVE,
            shared_state::CRITICAL,
            shared_state::EMPTY,
            shared_state::PLAYING,
            shared_state::STACKED,
            panel_action::FOCUS,
            panel_action::PRIMARY,
            panel_action::MUTED,
            panel_action::SEARCH,
            panel_action::CLOSE,
            panel_action::WITH_ICON,
            panel_action::ICON_ONLY,
            panel_action::ROW,
            panel_action::GROUP,
            panel_action::ROOT,
            panel_action::CONTENT,
            panel_action::GLYPH,
            panel_action::LABEL,
            panel_shell::WINDOW,
            panel_shell::ROOT,
            panel_shell::HEADER,
            panel_shell::HEADER_TOP,
            panel_shell::TITLE_STACK,
            panel_shell::TITLE_ROW,
            panel_shell::TITLE,
            panel_shell::COUNT,
            panel_shell::SEARCH,
            panel_shell::SEARCH_REVEALER,
            panel_shell::MEDIA_CONTAINER,
            panel_shell::QUICK_CONTROLS,
            panel_shell::WIDGET_STACK,
            panel_shell::WIDGET_REVEALER,
            panel_shell::TOGGLE_SECTION,
            panel_shell::STAT_SECTION,
            panel_shell::CARD_SECTION,
            panel_card::HAS_ACTIONS,
            panel_card::HAS_BODY,
            panel_card::HAS_SUMMARY,
            panel_card::NO_ACTIONS,
            toggle_card::GRID,
            toggle_card::ROOT,
            toggle_card::CONTENT,
            toggle_card::ICON,
            toggle_card::LABEL,
            toggle_card::HAS_ICON,
            toggle_card::NO_ICON,
            stat_card::GRID,
            stat_card::ROOT,
            stat_card::HEADER,
            stat_card::ICON,
            stat_card::TITLE,
            stat_card::VALUE,
            stat_card::BUILTIN,
            stat_card::PLUGIN,
            stat_card::HAS_ICON,
            stat_card::NO_ICON,
            info_card::GRID,
            info_card::ROOT,
            info_card::HEADER,
            info_card::ICON,
            info_card::TITLE,
            info_card::BODY,
            info_card::CALENDAR_WIDGET,
            info_card::CALENDAR,
            info_card::WEATHER,
            info_card::MONO,
            info_card::HAS_ICON,
            info_card::NO_ICON,
            popup_card::HAS_ACTIONS,
            popup_card::HAS_BODY,
            popup_card::HAS_ICON,
            popup_card::HAS_SUMMARY,
            popup_card::NO_ICON,
            group_row::ROOT,
            group_row::CONTAINER,
            group_row::HEADER,
            group_row::ICON,
            group_row::TITLE,
            group_row::COUNT,
            group_row::CHEVRON,
            group_row::COLLAPSED,
            group_row::EXPANDED,
            group_row::HAS_ICON,
            group_row::NO_ICON,
            empty_row::ROOT,
            empty_row::LABEL,
            ghost_row::ROOT,
            ghost_row::DEPTH_PREFIX,
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
