use unixnotis_core::Config;

use super::super::constants::{
    MEDIA_ART_FALLBACK_WIDTH_PX, MEDIA_ART_FRAME_FALLBACK_WIDTH_PX, MEDIA_BUTTON_FALLBACK_WIDTH_PX,
    MEDIA_NAV_FALLBACK_WIDTH_PX, MEDIA_TEXT_WIDTH_FLOOR_PX, WIDTH_WARNING_TOLERANCE_PX,
};
use super::super::{stock_config, stock_geometry_model, width_warning, GeometryModel};
use super::shell::{
    nav_cluster_spacing_px, ModeledMediaArtPosition, ModeledMediaControlsPosition,
    ModeledMediaNavigationPosition, ModeledMediaShell,
};

impl GeometryModel {
    pub(in super::super) fn media_width_warning(&self, config: &Config) -> Option<String> {
        if !config.media.enabled {
            // No media widget means no media width pressure
            return None;
        }
        // Width warnings compare the final shell pressure to the configured panel width
        let required_panel_width_px = self.media_required_panel_width_px(config);
        width_warning(
            "media row",
            required_panel_width_px,
            config.panel.width,
            "GTK natural width can still widen the panel when the media widget asks for more room",
        )
    }

    pub(in super::super) fn media_art_target_warning(&self) -> Option<String> {
        // Art width alone does not widen the card if the frame width stays smaller
        let art_width_px = self.media_art.outer_width_px(MEDIA_ART_FALLBACK_WIDTH_PX);
        let frame_width_px = self
            .media_art_frame
            .outer_width_px(MEDIA_ART_FRAME_FALLBACK_WIDTH_PX);
        if art_width_px <= frame_width_px + WIDTH_WARNING_TOLERANCE_PX {
            return None;
        }

        // The frame is what sets the outer slot width inside the card
        Some(format!(
            ".unixnotis-media-art now measures about {art_width_px}px while .unixnotis-media-art-frame is about {frame_width_px}px; the frame owns the outer card width, so picture-only sizing may not change the row the way the selector suggests"
        ))
    }

    fn media_required_panel_width_px(&self, config: &Config) -> i32 {
        // Stock pressure is used as the baseline because the final warning talks in panel width
        let pressure = self.media_pressure_px(config);
        let stock = stock_geometry_model();
        let stock_config = stock_config();
        let stock_pressure = stock.media_pressure_px(stock_config);

        // The delta from stock is easier to reason about than an absolute guess
        stock_config.panel.width + (pressure - stock_pressure)
    }

    fn media_pressure_px(&self, config: &Config) -> i32 {
        // The shell snapshot keeps the width math deterministic across helper calls
        let shell = ModeledMediaShell::from_config(&config.media);
        let text_width_px = config
            .panel
            .width
            .saturating_sub(media_text_reserve_px(shell))
            .max(shell.text_width_floor_px.max(MEDIA_TEXT_WIDTH_FLOOR_PX));
        // The text lane is fixed by the widget builder, but meta padding can still widen it
        let meta_width_px = self
            .media_meta
            .outer_width_px(text_width_px)
            .max(text_width_px);
        let text_width_px = self.media_text.outer_width_px(meta_width_px);

        // Transport width is the shared controls wrapper plus the three transport buttons
        let controls_width_px = self.media_controls.outer_insets_px()
            + (self
                .media_button
                .outer_width_px(MEDIA_BUTTON_FALLBACK_WIDTH_PX)
                * 3)
            + (shell.control_spacing_px * 2);
        let nav_button_width_px = self.media_nav.outer_width_px(MEDIA_NAV_FALLBACK_WIDTH_PX);
        let nav_inline_width_px = self.media_nav_strip.outer_insets_px()
            + (nav_button_width_px * 2)
            + shell.navigation_spacing_px;
        let nav_external_width_px = (nav_button_width_px * 2) + (shell.navigation_spacing_px * 2);
        let art_frame_width_px = self.media_art_frame.outer_width_px(
            shell
                .art_frame_size_px()
                .max(MEDIA_ART_FRAME_FALLBACK_WIDTH_PX),
        );

        // Body width grows as inline or side content gets attached to the text lane
        let mut body_content_width_px = text_width_px;
        let include_inline_controls =
            shell.controls_position == ModeledMediaControlsPosition::Inline;
        let include_inline_nav =
            shell.navigation_position == ModeledMediaNavigationPosition::Inline;
        if include_inline_controls || include_inline_nav {
            let mut inline_cluster_width_px = 0;
            if include_inline_controls {
                inline_cluster_width_px += controls_width_px;
            }
            if include_inline_nav {
                inline_cluster_width_px +=
                    nav_cluster_spacing_px(include_inline_controls, include_inline_nav, shell);
                inline_cluster_width_px += nav_inline_width_px;
            }
            body_content_width_px += shell.content_spacing_px + inline_cluster_width_px;
        }
        if let Some(side_rail_width_px) =
            self.media_side_rail_width_px(shell, controls_width_px, nav_inline_width_px)
        {
            body_content_width_px += shell.content_spacing_px + side_rail_width_px;
        }
        let body_width_px = self.media_body.outer_width_px(body_content_width_px);

        // Bottom strips stay under the body, so the main section only needs the wider of the two
        let bottom_strip_width_px =
            self.media_bottom_strip_width_px(shell, controls_width_px, nav_inline_width_px);
        let main_width_px = self
            .media_main
            .outer_width_px(body_width_px.max(bottom_strip_width_px));

        let mut header_content_width_px = main_width_px;
        if shell.art_position == ModeledMediaArtPosition::Start {
            header_content_width_px += shell.content_spacing_px + art_frame_width_px;
        }
        let header_width_px = self.media_header.outer_width_px(header_content_width_px);
        let card_content_width_px = if shell.art_position == ModeledMediaArtPosition::Top {
            // Top art competes with the header for the outer card width
            header_width_px.max(art_frame_width_px)
        } else {
            header_width_px
        };
        let card_outer_width_px = self.media_card.outer_width_px(card_content_width_px);

        self.panel.inner_insets_px()
            + self.media_container.outer_insets_px()
            + self.media_stack.outer_insets_px()
            + self.media_row.outer_insets_px()
            + card_outer_width_px
            + if shell.navigation_position == ModeledMediaNavigationPosition::External {
                nav_external_width_px
            } else {
                0
            }
    }

    fn media_side_rail_width_px(
        &self,
        shell: ModeledMediaShell,
        controls_width_px: i32,
        nav_inline_width_px: i32,
    ) -> Option<i32> {
        let include_controls = shell.controls_position == ModeledMediaControlsPosition::Side;
        let include_nav = shell.navigation_position == ModeledMediaNavigationPosition::Side;
        if !include_controls && !include_nav {
            return None;
        }

        // Side rails stack vertically, so the widest child becomes the rail width owner
        let content_width_px = match (include_controls, include_nav) {
            (true, true) => controls_width_px.max(nav_inline_width_px),
            (true, false) => controls_width_px,
            (false, true) => nav_inline_width_px,
            (false, false) => 0,
        };
        Some(self.media_action_rail.outer_width_px(content_width_px))
    }

    fn media_bottom_strip_width_px(
        &self,
        shell: ModeledMediaShell,
        controls_width_px: i32,
        nav_inline_width_px: i32,
    ) -> i32 {
        let include_controls = shell.controls_position == ModeledMediaControlsPosition::Bottom;
        let include_nav = shell.navigation_position == ModeledMediaNavigationPosition::Bottom;
        if !include_controls && !include_nav {
            return 0;
        }

        let mut content_width_px = 0;
        // Bottom strips run left to right, so both groups add onto the same width budget
        if include_controls {
            content_width_px += controls_width_px;
        }
        if include_nav {
            if content_width_px > 0 {
                content_width_px += nav_cluster_spacing_px(include_controls, include_nav, shell);
            }
            content_width_px += nav_inline_width_px;
        }
        self.media_control_strip.outer_width_px(content_width_px)
    }
}

pub(super) fn media_text_reserve_px(shell: ModeledMediaShell) -> i32 {
    // Reserve math mirrors the runtime shell so marquee width stays in sync with widget layout
    let mut reserve_px = 0;

    // Start art spends width beside the text lane while top art only affects height
    if shell.art_position == ModeledMediaArtPosition::Start {
        reserve_px += shell.art_frame_size_px() + shell.content_spacing_px;
    }

    let controls_width_px = (MEDIA_BUTTON_FALLBACK_WIDTH_PX * 3) + (shell.control_spacing_px * 2);
    let nav_inline_width_px = (MEDIA_NAV_FALLBACK_WIDTH_PX * 2) + shell.navigation_spacing_px;

    match shell.controls_position {
        ModeledMediaControlsPosition::Inline => {
            // Inline shells spend one outer gap plus one inner nav gap when both groups are shown
            let include_inline_controls = true;
            let include_inline_nav =
                shell.navigation_position == ModeledMediaNavigationPosition::Inline;
            let mut inline_cluster_width_px = controls_width_px;
            if include_inline_nav {
                inline_cluster_width_px +=
                    nav_cluster_spacing_px(include_inline_controls, include_inline_nav, shell);
                inline_cluster_width_px += nav_inline_width_px;
            }
            reserve_px += inline_cluster_width_px + shell.content_spacing_px;
        }
        ModeledMediaControlsPosition::Side => {
            // Side rails only need the widest vertical child, not the sum of both groups
            reserve_px += match shell.navigation_position {
                ModeledMediaNavigationPosition::Side => {
                    controls_width_px.max(nav_inline_width_px) + shell.content_spacing_px
                }
                _ => controls_width_px + shell.content_spacing_px,
            };
        }
        ModeledMediaControlsPosition::Bottom | ModeledMediaControlsPosition::Hidden => {}
    }

    if shell.navigation_position == ModeledMediaNavigationPosition::External {
        // External nav still consumes panel width even though it sits outside the card
        reserve_px += (MEDIA_NAV_FALLBACK_WIDTH_PX * 2) + (shell.navigation_spacing_px * 2);
    }

    reserve_px
}
