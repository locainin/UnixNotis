use unixnotis_core::Config;

use super::super::constants::{
    HEIGHT_WARNING_TOLERANCE_PX, MEDIA_ARTIST_FALLBACK_HEIGHT_PX,
    MEDIA_ART_FRAME_FALLBACK_WIDTH_PX, MEDIA_BUTTON_FALLBACK_HEIGHT_PX,
    MEDIA_META_LABEL_FALLBACK_HEIGHT_PX, MEDIA_NAV_FALLBACK_HEIGHT_PX, MEDIA_TEXT_ROW_SPACING_PX,
    MEDIA_TITLE_FALLBACK_HEIGHT_PX,
};
use super::super::GeometryModel;
use super::helpers::{append_vertical, max_optional_heights, stack_visible_heights};
use super::shell::{
    nav_cluster_spacing_px, ModeledMediaArtPosition, ModeledMediaControlsPosition,
    ModeledMediaNavigationPosition, ModeledMediaShell,
};

impl GeometryModel {
    pub(in super::super) fn media_height_warning(&self, config: &Config) -> Option<String> {
        if !config.media.enabled {
            return None;
        }

        // Height lint only runs for shells that can stack multiple vertical bands
        let shell = ModeledMediaShell::from_config(&config.media);
        if !shell.has_vertical_height_risk() {
            // The stock carousel shell already holds a stable one-band layout
            // Height warnings stay focused on multi-band shells that can overgrow the fixed card
            return None;
        }
        let required_card_height_px = self.media_required_card_height_px(config);
        if required_card_height_px <= shell.card_height_px + HEIGHT_WARNING_TOLERANCE_PX {
            // Small drift stays quiet so rounding noise does not become a warning
            return None;
        }

        Some(format!(
            "media card shell looks like it needs about {required_card_height_px}px of card height, but the effective media card height is {configured_height}px; top art, metadata, and transport rows may briefly allocate tall and then settle or clip depending on the theme",
            configured_height = shell.card_height_px
        ))
    }

    fn media_required_card_height_px(&self, config: &Config) -> i32 {
        // Height math follows the same shell routing the runtime widget uses
        let shell = ModeledMediaShell::from_config(&config.media);
        let art_frame_height_px = self.media_vertical.art_frame.outer_height_px(
            shell
                .art_frame_size_px()
                .max(MEDIA_ART_FRAME_FALLBACK_WIDTH_PX),
        );
        let button_height_px = self
            .media_vertical
            .button
            .outer_height_px(MEDIA_BUTTON_FALLBACK_HEIGHT_PX);
        let controls_height_px = self
            .media_vertical
            .controls
            .outer_height_px(button_height_px);
        let nav_height_px = self
            .media_vertical
            .nav
            .outer_height_px(MEDIA_NAV_FALLBACK_HEIGHT_PX);
        let nav_strip_height_px = self.media_vertical.nav_strip.outer_height_px(nav_height_px);
        let text_height_px = self.media_text_height_px(config);

        // Inline content competes with the text lane on the same row, so the tallest child wins
        let include_inline_controls =
            shell.controls_position == ModeledMediaControlsPosition::Inline;
        let include_inline_nav =
            shell.navigation_position == ModeledMediaNavigationPosition::Inline;
        let inline_height_px = max_optional_heights([
            include_inline_controls.then_some(controls_height_px),
            include_inline_nav.then_some(nav_strip_height_px),
        ]);
        let side_rail_height_px =
            self.media_side_rail_height_px(shell, controls_height_px, nav_strip_height_px);
        let body_content_height_px =
            max_optional_heights([Some(text_height_px), inline_height_px, side_rail_height_px])
                .unwrap_or(0);
        let body_height_px = self
            .media_vertical
            .body
            .outer_height_px(body_content_height_px);

        // Bottom strips stack under the body, so the heights add instead of compete
        let bottom_strip_height_px =
            self.media_bottom_strip_height_px(shell, controls_height_px, nav_strip_height_px);
        let main_content_height_px = append_vertical(
            body_height_px,
            bottom_strip_height_px,
            shell.content_spacing_px,
        )
        .unwrap_or(body_height_px);
        let main_height_px = self
            .media_vertical
            .main
            .outer_height_px(main_content_height_px);

        let header_content_height_px = if shell.art_position == ModeledMediaArtPosition::Start {
            art_frame_height_px.max(main_height_px)
        } else {
            main_height_px
        };
        let header_height_px = self
            .media_vertical
            .header
            .outer_height_px(header_content_height_px);
        let card_content_height_px = if shell.art_position == ModeledMediaArtPosition::Top {
            // Top art turns the card into a two-band vertical stack
            append_vertical(
                art_frame_height_px,
                Some(header_height_px),
                shell.content_spacing_px,
            )
            .unwrap_or(art_frame_height_px)
        } else {
            header_height_px
        };

        self.media_vertical
            .card
            .outer_height_px(card_content_height_px)
    }

    fn media_side_rail_height_px(
        &self,
        shell: ModeledMediaShell,
        controls_height_px: i32,
        nav_strip_height_px: i32,
    ) -> Option<i32> {
        let include_controls = shell.controls_position == ModeledMediaControlsPosition::Side;
        let include_nav = shell.navigation_position == ModeledMediaNavigationPosition::Side;
        if !include_controls && !include_nav {
            return None;
        }

        // Side rails stack transport and nav one above the other when both are active
        let content_height_px = append_vertical(
            include_controls.then_some(controls_height_px),
            include_nav.then_some(nav_strip_height_px),
            nav_cluster_spacing_px(include_controls, include_nav, shell),
        )
        .unwrap_or(0);
        Some(
            self.media_vertical
                .action_rail
                .outer_height_px(content_height_px),
        )
    }

    fn media_bottom_strip_height_px(
        &self,
        shell: ModeledMediaShell,
        controls_height_px: i32,
        nav_strip_height_px: i32,
    ) -> Option<i32> {
        let include_controls = shell.controls_position == ModeledMediaControlsPosition::Bottom;
        let include_nav = shell.navigation_position == ModeledMediaNavigationPosition::Bottom;
        if !include_controls && !include_nav {
            return None;
        }

        // Bottom strips share one row, so the taller child sets the row height
        let content_height_px = max_optional_heights([
            include_controls.then_some(controls_height_px),
            include_nav.then_some(nav_strip_height_px),
        ])
        .unwrap_or(0);
        Some(
            self.media_vertical
                .control_strip
                .outer_height_px(content_height_px),
        )
    }

    fn media_text_height_px(&self, config: &Config) -> i32 {
        // The text stack only counts rows the runtime can actually show
        let meta_height_px = if config.media.show_source || config.media.show_position {
            let label_height_px = max_optional_heights([
                config.media.show_source.then_some(
                    self.media_vertical
                        .source
                        .outer_height_px(MEDIA_META_LABEL_FALLBACK_HEIGHT_PX),
                ),
                config.media.show_position.then_some(
                    self.media_vertical
                        .position
                        .outer_height_px(MEDIA_META_LABEL_FALLBACK_HEIGHT_PX),
                ),
            ])
            .unwrap_or(0);
            Some(self.media_vertical.meta.outer_height_px(label_height_px))
        } else {
            None
        };
        let title_height_px = config.media.show_title.then_some(
            self.media_vertical
                .title
                .outer_height_px(MEDIA_TITLE_FALLBACK_HEIGHT_PX),
        );
        let artist_height_px = config.media.show_artist.then_some(
            self.media_vertical
                .artist
                .outer_height_px(MEDIA_ARTIST_FALLBACK_HEIGHT_PX),
        );
        // Text rows stack vertically with the same small gap used by the widget theme
        let text_content_height_px = stack_visible_heights(
            &[meta_height_px, title_height_px, artist_height_px],
            MEDIA_TEXT_ROW_SPACING_PX,
        );
        self.media_vertical
            .text
            .outer_height_px(text_content_height_px)
    }
}
