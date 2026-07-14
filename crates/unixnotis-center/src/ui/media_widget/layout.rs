use unixnotis_core::MediaLayout;

use super::shell::{
    MediaShellConfig, ResolvedMediaArtPosition, ResolvedMediaControlsPosition,
    ResolvedMediaNavigationPosition,
};

const MIN_MEDIA_TEXT_WIDTH_FLOOR_PX: i32 = 48;
// GTK applies the panel request to the styled root's content box
// Media therefore receives that width directly without subtracting panel padding again
const MEDIA_BUTTON_FALLBACK_WIDTH_PX: i32 = 38;
const MEDIA_NAV_FALLBACK_WIDTH_PX: i32 = 24;
const MEDIA_ART_FRAME_EXTRA_PX: i32 = 4;
// The stock card adds ten pixels of padding and one border pixel on each side
// Reserving that outer chrome stops the card's minimum width from growing the panel
const MEDIA_CARD_HORIZONTAL_CHROME_PX: i32 = 22;

pub(super) const fn stack_layout_class(layout: MediaLayout) -> &'static str {
    // Stable classes let media.css style each shell without guessing structure
    match layout {
        MediaLayout::Carousel => "unixnotis-media-stack-carousel",
        MediaLayout::Inline => "unixnotis-media-stack-inline",
        MediaLayout::Stacked => "unixnotis-media-stack-stacked",
        MediaLayout::Showcase => "unixnotis-media-stack-showcase",
        MediaLayout::Player => "unixnotis-media-stack-player",
    }
}

pub(super) const fn row_layout_class(layout: MediaLayout) -> &'static str {
    // Row classes mirror the shell preset so width tweaks can stay layout specific
    match layout {
        MediaLayout::Carousel => "unixnotis-media-row-carousel",
        MediaLayout::Inline => "unixnotis-media-row-inline",
        MediaLayout::Stacked => "unixnotis-media-row-stacked",
        MediaLayout::Showcase => "unixnotis-media-row-showcase",
        MediaLayout::Player => "unixnotis-media-row-player",
    }
}

pub(super) const fn card_layout_class(layout: MediaLayout) -> &'static str {
    // Card classes are the main theme hook users touch when ricing the player
    match layout {
        MediaLayout::Carousel => "unixnotis-media-card-carousel",
        MediaLayout::Inline => "unixnotis-media-card-inline",
        MediaLayout::Stacked => "unixnotis-media-card-stacked",
        MediaLayout::Showcase => "unixnotis-media-card-showcase",
        MediaLayout::Player => "unixnotis-media-card-player",
    }
}

pub(super) fn media_content_width(panel_width: i32) -> i32 {
    panel_width
        // Tiny or invalid widths still need a positive allocation target
        .max(1)
}

pub(super) fn marquee_width_for_shell(shell: &MediaShellConfig, panel_width: i32) -> i32 {
    marquee_width_for_shell_player_count(shell, panel_width, true)
}

pub(super) fn marquee_width_for_shell_player_count(
    shell: &MediaShellConfig,
    panel_width: i32,
    has_multiple_players: bool,
) -> i32 {
    let reserve_px = media_text_reserve_px(shell, has_multiple_players);
    media_content_width(panel_width)
        .saturating_sub(reserve_px)
        // The configured floor remains bounded by the hard safety floor when space is exhausted
        .max(shell.text_width_floor_px.min(MIN_MEDIA_TEXT_WIDTH_FLOOR_PX))
}

pub(super) const fn card_height_for_shell(shell: &MediaShellConfig) -> i32 {
    shell.card_height_px
}

pub(super) const fn art_frame_size_px(shell: &MediaShellConfig) -> i32 {
    shell.art_size_px.saturating_add(MEDIA_ART_FRAME_EXTRA_PX)
}

fn media_text_reserve_px(shell: &MediaShellConfig, has_multiple_players: bool) -> i32 {
    // Text shares the card's inner allocation, so card padding and borders consume width first
    let mut reserve_px = MEDIA_CARD_HORIZONTAL_CHROME_PX;

    if shell.art_position == ResolvedMediaArtPosition::Start {
        reserve_px += art_frame_size_px(shell) + shell.content_spacing_px;
    }

    let controls_width_px =
        (MEDIA_BUTTON_FALLBACK_WIDTH_PX * 3) + (shell.control_spacing_px.saturating_mul(2));
    let nav_width_px = (MEDIA_NAV_FALLBACK_WIDTH_PX * 2) + shell.navigation_spacing_px;

    match shell.controls_position {
        ResolvedMediaControlsPosition::Inline => {
            reserve_px += controls_width_px + shell.content_spacing_px;
            if shell.navigation_position == ResolvedMediaNavigationPosition::Inline {
                reserve_px += nav_width_px + shell.navigation_spacing_px;
            }
        }
        ResolvedMediaControlsPosition::Side => {
            let side_width_px = match shell.navigation_position {
                ResolvedMediaNavigationPosition::Side => {
                    controls_width_px.max(nav_width_px) + shell.content_spacing_px
                }
                _ => controls_width_px + shell.content_spacing_px,
            };
            reserve_px += side_width_px;
        }
        ResolvedMediaControlsPosition::Bottom | ResolvedMediaControlsPosition::Hidden => {}
    }

    if has_multiple_players
        && shell.navigation_position == ResolvedMediaNavigationPosition::External
    {
        reserve_px += nav_width_px + shell.navigation_spacing_px;
    }

    reserve_px
}

#[cfg(test)]
#[path = "tests/layout.rs"]
mod tests;
