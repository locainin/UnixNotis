//! Public CSS hook consistency tests

use std::collections::HashSet;

use super::{
    cut_corner, dnd_menu, empty_row, group_row, info_card, media_card, media_shell, panel_action,
    panel_card, panel_shell, popup_card, shared_state, slider, stat_card, toggle_card, urgency,
};

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one flat hook inventory makes duplicate public CSS names visible in a single assertion"
)]
fn hook_names_stay_unique() {
    // One flat set makes accidental selector reuse obvious during refactors
    let names = [
        cut_corner::ROOT,
        dnd_menu::ROOT,
        dnd_menu::CONTENT,
        dnd_menu::TITLE,
        dnd_menu::CHOICE,
        dnd_menu::INDEFINITE,
        dnd_menu::SEPARATOR,
        shared_state::ACTIVE,
        shared_state::CRITICAL,
        shared_state::EMPTY,
        shared_state::PLAYING,
        shared_state::COLLAPSED_GROUP_PREVIEW,
        urgency::BADGE,
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
        panel_action::LABEL_HIDDEN,
        panel_shell::WINDOW,
        panel_shell::ROOT,
        panel_shell::REDUCED_MOTION,
        panel_shell::HEADER,
        panel_shell::HEADER_TOP,
        panel_shell::TITLE_STACK,
        panel_shell::TITLE_ROW,
        panel_shell::TITLE,
        panel_shell::SUBTITLE,
        panel_shell::COUNT,
        panel_shell::SEARCH,
        panel_shell::SEARCH_MAGNIFIER,
        panel_shell::SEARCH_CLEAR,
        panel_shell::SEARCH_OWNED_ICONS,
        panel_shell::SEARCH_SHELL,
        panel_shell::SEARCH_ACCENT,
        panel_shell::SEARCH_STAR,
        panel_shell::SEARCH_REVEALER,
        panel_shell::RELOAD_NOTICE,
        panel_shell::RELOAD_NOTICE_ERROR,
        panel_shell::RELOAD_NOTICE_WARNING,
        panel_shell::RELOAD_NOTICE_CONTENT,
        panel_shell::RELOAD_NOTICE_TEXT,
        panel_shell::RELOAD_NOTICE_CLOSE,
        panel_shell::BODY_STACK,
        panel_shell::EDGE_TOP,
        panel_shell::EDGE_BOTTOM,
        panel_shell::RAIL_LEFT,
        panel_shell::RAIL_RIGHT,
        panel_shell::TICK_TOP_LEFT,
        panel_shell::TICK_TOP_RIGHT,
        panel_shell::TICK_BOTTOM_LEFT,
        panel_shell::TICK_BOTTOM_RIGHT,
        panel_shell::MEDIA_CONTAINER,
        panel_shell::QUICK_CONTROLS,
        panel_shell::WIDGET_STACK,
        panel_shell::WIDGET_DENSITY_COMFORTABLE,
        panel_shell::WIDGET_DENSITY_COMPACT,
        panel_shell::WIDGET_REVEALER,
        panel_shell::SECTION_HEADER,
        panel_shell::RECENT_SECTION,
        panel_shell::RECENT_HEADER,
        panel_shell::RECENT_HEADER_ROW,
        panel_shell::FOOTER,
        panel_shell::TOGGLE_SECTION,
        panel_shell::STAT_SECTION,
        panel_shell::CARD_SECTION,
        panel_card::ROW,
        panel_card::HEADER,
        panel_card::TEXT,
        panel_card::META_TOP,
        panel_card::META_LABEL,
        panel_card::TIME_BADGE,
        panel_card::FOOTER,
        panel_card::FOOTER_LEFT,
        panel_card::FOOTER_RIGHT,
        panel_card::THUMBNAIL,
        panel_card::GROUPED,
        panel_card::HAS_ACTIONS,
        panel_card::HAS_BODY,
        panel_card::HAS_SUMMARY,
        panel_card::HAS_THUMBNAIL,
        panel_card::NO_ACTIONS,
        panel_card::NO_THUMBNAIL,
        slider::ROOT,
        slider::ICON,
        slider::SCALE,
        slider::VALUE,
        slider::STACK,
        slider::SEGMENTS,
        slider::SEGMENT,
        slider::SUBLABEL_ROW,
        slider::SUBLABEL_MIN,
        slider::SUBLABEL_MAX,
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
        info_card::MEDIA,
        info_card::CHROME,
        info_card::DOTS,
        info_card::DOT,
        info_card::NAV_PREV,
        info_card::NAV_NEXT,
        info_card::CALENDAR_WIDGET,
        info_card::CALENDAR,
        info_card::WEATHER,
        info_card::MONO,
        info_card::LAYOUT_BANNER,
        info_card::LAYOUT_IMAGE_ROW,
        info_card::HAS_ICON,
        info_card::NO_ICON,
        popup_card::HAS_ACTIONS,
        popup_card::HAS_BODY,
        popup_card::HAS_ICON,
        popup_card::HAS_IMAGE,
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
        media_card::EMPTY_ARTIST,
        media_card::HAS_ART,
        media_card::HAS_ARTIST,
        media_card::MULTI_PLAYER,
        media_card::NO_ART,
        media_card::PAUSED,
        media_card::PLAYING,
        media_card::SINGLE_PLAYER,
        media_card::STOPPED,
        media_shell::STACK,
        media_shell::ROW,
        media_shell::CARD,
        media_shell::HEADER,
        media_shell::BODY,
        media_shell::TEXT,
        media_shell::META,
        media_shell::SOURCE,
        media_shell::POSITION,
        media_shell::TITLE,
        media_shell::ARTIST,
        media_shell::ART,
        media_shell::ART_FRAME,
        media_shell::MAIN,
        media_shell::CONTROLS,
        media_shell::CONTROL_STRIP,
        media_shell::ACTION_RAIL,
        media_shell::NAV_STRIP,
        media_shell::NAV,
        media_shell::NAV_PREV,
        media_shell::NAV_NEXT,
        media_shell::BUTTON,
        media_shell::BUTTON_PREV,
        media_shell::BUTTON_PLAY,
        media_shell::BUTTON_NEXT,
        media_shell::HAS_TITLE,
        media_shell::NO_TITLE,
        media_shell::HAS_SOURCE,
        media_shell::NO_SOURCE,
        media_shell::HAS_POSITION,
        media_shell::NO_POSITION,
        media_shell::HAS_CONTROLS,
        media_shell::NO_CONTROLS,
        media_shell::HAS_NAV,
        media_shell::NO_NAV,
        media_shell::ART_START,
        media_shell::ART_TOP,
        media_shell::ART_HIDDEN,
        media_shell::CONTROLS_INLINE,
        media_shell::CONTROLS_BOTTOM,
        media_shell::CONTROLS_SIDE,
        media_shell::CONTROLS_HIDDEN,
        media_shell::NAV_EXTERNAL,
        media_shell::NAV_INLINE,
        media_shell::NAV_BOTTOM,
        media_shell::NAV_SIDE,
        media_shell::NAV_HIDDEN,
    ];
    let unique = names.iter().copied().collect::<HashSet<_>>();
    assert_eq!(unique.len(), names.len());
}

#[test]
fn stock_panel_css_targets_real_group_card_hooks() {
    let css = crate::theme::DEFAULT_PANEL_CSS;

    // Group headers and notification cards are sibling ListView rows, not nested widgets
    // Grouped cards stay separate while collapsed previews own their internal depth layers
    assert!(css.contains(&format!(".unixnotis-panel-card.{}", panel_card::GROUPED)));
    assert!(css.contains(&format!(
        ".unixnotis-panel-card-foreground.{}",
        panel_card::GROUPED
    )));
    assert!(css.contains(&format!(
        ".unixnotis-panel-card.{}",
        shared_state::COLLAPSED_GROUP_PREVIEW
    )));
    assert!(css.contains(".unixnotis-stack-layer-back"));
    assert!(css.contains(".unixnotis-stack-layer-middle"));

    // These selectors belonged to an older nested-card idea and do not match the real tree
    assert!(!css.contains("unixnotis-group-cards"));
    assert!(!css.contains(".unixnotis-group .unixnotis-panel-card"));
    assert!(!css.contains(".unixnotis-group-row-collapsed .unixnotis-panel-card"));
}

#[test]
fn stock_group_count_stays_neutral_during_header_hover() {
    let css = crate::theme::DEFAULT_PANEL_CSS;

    assert!(css.contains(
        ".unixnotis-group-header:hover .unixnotis-group-count {\n  background: alpha(#ffffff, 0.09);"
    ));
    assert!(!css.contains(
        ".unixnotis-group-header:hover .unixnotis-group-count {\n  background: alpha(@unixnotis-accent"
    ));
}

#[test]
fn stock_panel_css_uses_separated_rows_and_bounded_depth_layers() {
    let css = crate::theme::DEFAULT_PANEL_CSS;

    // The discarded legacy names stay absent while the new layers remain explicit
    assert!(!css.contains("unixnotis-stack-ghost"));
    assert!(css.contains(".unixnotis-stack-layer-back"));
    assert!(css.contains(".unixnotis-stack-layer-middle"));
    assert!(css.contains("margin: 6px 14px 0"));
    assert!(css.contains("margin: 12px 8px 8px"));
    assert!(!css.contains("margin: -58px 14px 0"));
    assert!(css.contains("margin: 0 20px"));
    assert!(css.contains(".unixnotis-panel-card.unixnotis-panel-card-grouped {\n  border-radius:"));
}

#[test]
fn stock_panel_close_control_remains_available_without_hover() {
    let css = crate::theme::DEFAULT_PANEL_CSS;

    assert!(css.contains(".unixnotis-panel-close {\n"));
    assert!(!css.contains(".unixnotis-panel-card-overlay:hover .unixnotis-panel-close"));
    assert!(css.contains(".unixnotis-panel-close:hover"));
    assert!(css.contains(".unixnotis-panel-close:focus-visible"));
}
