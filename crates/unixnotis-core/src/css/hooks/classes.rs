//! Stable CSS class names shared by widgets and themes

pub mod shared_state {
    // Short state names appear in many selectors and mirror GTK-style class usage
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
    pub const LABEL_HIDDEN: &str = "unixnotis-panel-action-label-hidden";
}

pub mod panel_shell {
    // Panel shell hooks keep split panel files on one stable class contract
    pub const WINDOW: &str = "unixnotis-panel-window";
    pub const ROOT: &str = "unixnotis-panel";
    pub const HEADER: &str = "unixnotis-panel-header";
    pub const HEADER_TOP: &str = "unixnotis-panel-header-top";
    pub const TITLE_STACK: &str = "unixnotis-panel-title-stack";
    pub const TITLE_ROW: &str = "unixnotis-panel-title-row";
    pub const TITLE: &str = "unixnotis-panel-title";
    pub const SUBTITLE: &str = "unixnotis-panel-subtitle";
    pub const COUNT: &str = "unixnotis-panel-count";
    pub const SEARCH: &str = "unixnotis-panel-search";
    pub const SEARCH_SHELL: &str = "unixnotis-panel-search-shell";
    pub const SEARCH_ACCENT: &str = "unixnotis-panel-search-accent";
    pub const SEARCH_STAR: &str = "unixnotis-panel-search-star";
    pub const SEARCH_REVEALER: &str = "unixnotis-panel-search-revealer";
    pub const BODY_STACK: &str = "unixnotis-panel-body-stack";
    pub const EDGE_TOP: &str = "unixnotis-panel-edge-top";
    pub const EDGE_BOTTOM: &str = "unixnotis-panel-edge-bottom";
    pub const RAIL_LEFT: &str = "unixnotis-panel-rail-left";
    pub const RAIL_RIGHT: &str = "unixnotis-panel-rail-right";
    pub const TICK_TOP_LEFT: &str = "unixnotis-panel-tick-top-left";
    pub const TICK_TOP_RIGHT: &str = "unixnotis-panel-tick-top-right";
    pub const TICK_BOTTOM_LEFT: &str = "unixnotis-panel-tick-bottom-left";
    pub const TICK_BOTTOM_RIGHT: &str = "unixnotis-panel-tick-bottom-right";
    pub const MEDIA_CONTAINER: &str = "unixnotis-media-container";
    pub const QUICK_CONTROLS: &str = "unixnotis-quick-controls";
    pub const WIDGET_STACK: &str = "unixnotis-widget-stack";
    pub const WIDGET_REVEALER: &str = "unixnotis-widget-revealer";
    pub const SECTION_HEADER: &str = "unixnotis-section-header";
    pub const RECENT_SECTION: &str = "unixnotis-recent-section";
    pub const RECENT_HEADER: &str = "unixnotis-recent-header";
    pub const RECENT_HEADER_ROW: &str = "unixnotis-recent-header-row";
    pub const FOOTER: &str = "unixnotis-panel-footer";
    pub const TOGGLE_SECTION: &str = "unixnotis-toggle-section";
    pub const STAT_SECTION: &str = "unixnotis-stat-section";
    pub const CARD_SECTION: &str = "unixnotis-card-section";
}

pub mod panel_card {
    pub const ROW: &str = "unixnotis-panel-card-row";
    pub const HEADER: &str = "unixnotis-panel-card-header";
    pub const TEXT: &str = "unixnotis-panel-card-text";
    pub const META_TOP: &str = "unixnotis-panel-card-meta-top";
    pub const META_LABEL: &str = "unixnotis-panel-card-meta-label";
    pub const TIME_BADGE: &str = "unixnotis-panel-card-time-badge";
    pub const FOOTER: &str = "unixnotis-panel-card-footer";
    pub const FOOTER_LEFT: &str = "unixnotis-panel-card-footer-left";
    pub const FOOTER_RIGHT: &str = "unixnotis-panel-card-footer-right";
    pub const THUMBNAIL: &str = "unixnotis-panel-card-thumbnail";
    pub const GROUP_COLLAPSED: &str = "unixnotis-panel-card-group-collapsed";
    pub const GROUP_EXPANDED: &str = "unixnotis-panel-card-group-expanded";
    pub const GROUPED: &str = "unixnotis-panel-card-grouped";
    pub const HAS_ACTIONS: &str = "unixnotis-panel-card-has-actions";
    pub const HAS_BODY: &str = "unixnotis-panel-card-has-body";
    pub const HAS_SUMMARY: &str = "unixnotis-panel-card-has-summary";
    pub const HAS_THUMBNAIL: &str = "unixnotis-panel-card-has-thumbnail";
    pub const NO_ACTIONS: &str = "unixnotis-panel-card-no-actions";
    pub const NO_THUMBNAIL: &str = "unixnotis-panel-card-no-thumbnail";
}

pub mod slider {
    pub const ROOT: &str = "unixnotis-quick-slider";
    pub const ICON: &str = "unixnotis-quick-slider-icon";
    pub const SCALE: &str = "unixnotis-quick-slider-scale";
    pub const VALUE: &str = "unixnotis-quick-slider-value";
    pub const STACK: &str = "unixnotis-quick-slider-stack";
    pub const SEGMENTS: &str = "unixnotis-quick-slider-segments";
    pub const SEGMENT: &str = "unixnotis-quick-slider-segment";
    pub const SUBLABEL_ROW: &str = "unixnotis-quick-slider-sublabel-row";
    pub const SUBLABEL_MIN: &str = "unixnotis-quick-slider-sublabel-min";
    pub const SUBLABEL_MAX: &str = "unixnotis-quick-slider-sublabel-max";
}

pub mod toggle_card {
    pub const GRID: &str = "unixnotis-toggle-grid";
    pub const ROOT: &str = "unixnotis-toggle";
    pub const CONTENT: &str = "unixnotis-toggle-content";
    pub const ICON: &str = "unixnotis-toggle-icon";
    pub const LABEL: &str = "unixnotis-toggle-label";
    pub const HAS_ICON: &str = "unixnotis-toggle-has-icon";
    pub const NO_ICON: &str = "unixnotis-toggle-no-icon";
}

pub mod stat_card {
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
    pub const GRID: &str = "unixnotis-card-grid";
    pub const ROOT: &str = "unixnotis-info-card";
    pub const HEADER: &str = "unixnotis-info-header";
    pub const ICON: &str = "unixnotis-info-icon";
    pub const TITLE: &str = "unixnotis-info-title";
    pub const BODY: &str = "unixnotis-info-body";
    pub const MEDIA: &str = "unixnotis-info-media";
    pub const CHROME: &str = "unixnotis-info-chrome";
    pub const DOTS: &str = "unixnotis-info-dots";
    pub const DOT: &str = "unixnotis-info-dot";
    pub const NAV_PREV: &str = "unixnotis-info-nav-prev";
    pub const NAV_NEXT: &str = "unixnotis-info-nav-next";
    pub const CALENDAR_WIDGET: &str = "unixnotis-calendar";
    pub const CALENDAR: &str = "unixnotis-info-card-calendar";
    pub const WEATHER: &str = "unixnotis-info-card-weather";
    pub const MONO: &str = "unixnotis-info-card-mono";
    pub const LAYOUT_BANNER: &str = "unixnotis-info-card-banner";
    pub const LAYOUT_IMAGE_ROW: &str = "unixnotis-info-card-image-row";
    pub const HAS_ICON: &str = "unixnotis-info-card-has-icon";
    pub const NO_ICON: &str = "unixnotis-info-card-no-icon";
}

pub mod popup_card {
    pub const HAS_ACTIONS: &str = "unixnotis-popup-card-has-actions";
    pub const HAS_BODY: &str = "unixnotis-popup-card-has-body";
    pub const HAS_ICON: &str = "unixnotis-popup-card-has-icon";
    pub const HAS_SUMMARY: &str = "unixnotis-popup-card-has-summary";
    pub const NO_ICON: &str = "unixnotis-popup-card-no-icon";
}

pub mod group_row {
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
    pub const ROOT: &str = "unixnotis-empty";
    pub const LABEL: &str = "unixnotis-empty-label";
}

pub mod ghost_row {
    pub const ROOT: &str = "unixnotis-stack-ghost";
    pub const DEPTH_PREFIX: &str = "unixnotis-stack-ghost-";
}

pub mod media_card {
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

pub mod media_shell {
    pub const STACK: &str = "unixnotis-media-stack";
    pub const ROW: &str = "unixnotis-media-row";
    pub const CARD: &str = "unixnotis-media-card";
    pub const HEADER: &str = "unixnotis-media-header";
    pub const BODY: &str = "unixnotis-media-body";
    pub const TEXT: &str = "unixnotis-media-text";
    pub const META: &str = "unixnotis-media-meta";
    pub const SOURCE: &str = "unixnotis-media-source";
    pub const POSITION: &str = "unixnotis-media-position";
    pub const TITLE: &str = "unixnotis-media-title";
    pub const ARTIST: &str = "unixnotis-media-artist";
    pub const ART: &str = "unixnotis-media-art";
    pub const ART_FRAME: &str = "unixnotis-media-art-frame";
    pub const MAIN: &str = "unixnotis-media-main";
    pub const CONTROLS: &str = "unixnotis-media-controls";
    pub const CONTROL_STRIP: &str = "unixnotis-media-control-strip";
    pub const ACTION_RAIL: &str = "unixnotis-media-action-rail";
    pub const NAV_STRIP: &str = "unixnotis-media-nav-strip";
    pub const NAV: &str = "unixnotis-media-nav";
    pub const NAV_PREV: &str = "unixnotis-media-nav-prev";
    pub const NAV_NEXT: &str = "unixnotis-media-nav-next";
    pub const BUTTON: &str = "unixnotis-media-button";
    pub const BUTTON_PREV: &str = "unixnotis-media-button-prev";
    pub const BUTTON_PLAY: &str = "unixnotis-media-button-play";
    pub const BUTTON_NEXT: &str = "unixnotis-media-button-next";
    pub const HAS_TITLE: &str = "unixnotis-media-has-title";
    pub const NO_TITLE: &str = "unixnotis-media-no-title";
    pub const HAS_SOURCE: &str = "unixnotis-media-has-source";
    pub const NO_SOURCE: &str = "unixnotis-media-no-source";
    pub const HAS_POSITION: &str = "unixnotis-media-has-position";
    pub const NO_POSITION: &str = "unixnotis-media-no-position";
    pub const HAS_CONTROLS: &str = "unixnotis-media-has-controls";
    pub const NO_CONTROLS: &str = "unixnotis-media-no-controls";
    pub const HAS_NAV: &str = "unixnotis-media-has-nav";
    pub const NO_NAV: &str = "unixnotis-media-no-nav";
    pub const ART_START: &str = "unixnotis-media-art-start";
    pub const ART_TOP: &str = "unixnotis-media-art-top";
    pub const ART_HIDDEN: &str = "unixnotis-media-art-hidden";
    pub const CONTROLS_INLINE: &str = "unixnotis-media-controls-inline";
    pub const CONTROLS_BOTTOM: &str = "unixnotis-media-controls-bottom";
    pub const CONTROLS_SIDE: &str = "unixnotis-media-controls-side";
    pub const CONTROLS_HIDDEN: &str = "unixnotis-media-controls-hidden";
    pub const NAV_EXTERNAL: &str = "unixnotis-media-nav-external";
    pub const NAV_INLINE: &str = "unixnotis-media-nav-inline";
    pub const NAV_BOTTOM: &str = "unixnotis-media-nav-bottom";
    pub const NAV_SIDE: &str = "unixnotis-media-nav-side";
    pub const NAV_HIDDEN: &str = "unixnotis-media-nav-hidden";
}
