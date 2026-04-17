use unixnotis_core::MediaLayout;

use super::shell::{
    MediaShellConfig, ResolvedMediaArtPosition, ResolvedMediaControlsPosition,
    ResolvedMediaNavigationPosition,
};

const MIN_MEDIA_TEXT_WIDTH_FLOOR_PX: i32 = 48;
// Panel width is measured on the outer surface, but media lives inside the panel body
// Default panel padding plus a little border slack needs to be removed first
const PANEL_SURFACE_HORIZONTAL_CHROME_PX: i32 = 36;
const MEDIA_BUTTON_FALLBACK_WIDTH_PX: i32 = 28;
const MEDIA_NAV_FALLBACK_WIDTH_PX: i32 = 22;
const MEDIA_ART_FRAME_EXTRA_PX: i32 = 4;

pub(super) fn stack_layout_class(layout: MediaLayout) -> &'static str {
    // Stable classes let media.css style each shell without guessing structure
    match layout {
        MediaLayout::Carousel => "unixnotis-media-stack-carousel",
        MediaLayout::Inline => "unixnotis-media-stack-inline",
        MediaLayout::Stacked => "unixnotis-media-stack-stacked",
        MediaLayout::Showcase => "unixnotis-media-stack-showcase",
    }
}

pub(super) fn row_layout_class(layout: MediaLayout) -> &'static str {
    // Row classes mirror the shell preset so width tweaks can stay layout specific
    match layout {
        MediaLayout::Carousel => "unixnotis-media-row-carousel",
        MediaLayout::Inline => "unixnotis-media-row-inline",
        MediaLayout::Stacked => "unixnotis-media-row-stacked",
        MediaLayout::Showcase => "unixnotis-media-row-showcase",
    }
}

pub(super) fn card_layout_class(layout: MediaLayout) -> &'static str {
    // Card classes are the main theme hook users touch when ricing the player
    match layout {
        MediaLayout::Carousel => "unixnotis-media-card-carousel",
        MediaLayout::Inline => "unixnotis-media-card-inline",
        MediaLayout::Stacked => "unixnotis-media-card-stacked",
        MediaLayout::Showcase => "unixnotis-media-card-showcase",
    }
}

pub(super) fn media_content_width(panel_width: i32) -> i32 {
    panel_width
        .saturating_sub(PANEL_SURFACE_HORIZONTAL_CHROME_PX)
        // Tiny or invalid widths still need a positive allocation target
        .max(1)
}

pub(super) fn marquee_width_for_shell(shell: &MediaShellConfig, panel_width: i32) -> i32 {
    let reserve_px = media_text_reserve_px(shell);
    media_content_width(panel_width)
        .saturating_sub(reserve_px)
        // Tiny panel widths still keep a minimum readable title lane
        .max(shell.text_width_floor_px.max(MIN_MEDIA_TEXT_WIDTH_FLOOR_PX))
}

pub(super) fn card_height_for_shell(shell: &MediaShellConfig) -> i32 {
    shell.card_height_px
}

pub(super) fn art_frame_size_px(shell: &MediaShellConfig) -> i32 {
    shell.art_size_px.saturating_add(MEDIA_ART_FRAME_EXTRA_PX)
}

fn media_text_reserve_px(shell: &MediaShellConfig) -> i32 {
    let mut reserve_px = 0;

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

    if shell.navigation_position == ResolvedMediaNavigationPosition::External {
        reserve_px += nav_width_px + shell.navigation_spacing_px;
    }

    reserve_px
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
