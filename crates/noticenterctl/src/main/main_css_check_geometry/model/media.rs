use unixnotis_core::{Config, MediaLayout};

use super::constants::{
    MEDIA_ACTION_RAIL_SPACING_PX, MEDIA_ART_FALLBACK_WIDTH_PX, MEDIA_ART_FRAME_FALLBACK_WIDTH_PX,
    MEDIA_BUTTON_FALLBACK_WIDTH_PX, MEDIA_CARD_CONTENT_SPACING_PX, MEDIA_CAROUSEL_TEXT_RESERVE_PX,
    MEDIA_CONTROL_BUTTON_SPACING_PX, MEDIA_INLINE_TEXT_RESERVE_PX, MEDIA_NAV_FALLBACK_WIDTH_PX,
    MEDIA_ROW_SPACING_PX, MEDIA_SHOWCASE_TEXT_RESERVE_PX, MEDIA_STACKED_TEXT_RESERVE_PX,
    MEDIA_TEXT_WIDTH_FLOOR_PX, WIDTH_WARNING_TOLERANCE_PX,
};
use super::{stock_config, stock_geometry_model, width_warning, GeometryModel};

// Media keeps its own file because the row width rules are much more specific
impl GeometryModel {
    pub(super) fn media_width_warning(&self, config: &Config) -> Option<String> {
        if !config.media.enabled {
            // No media widget means no media width pressure
            return None;
        }
        let required_panel_width_px = self.media_required_panel_width_px(config);
        width_warning(
            "media row",
            required_panel_width_px,
            config.panel.width,
            "GTK natural width can still widen the panel when the media widget asks for more room",
        )
    }

    pub(super) fn media_art_target_warning(&self) -> Option<String> {
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
        let pressure = self.media_pressure_px(config.panel.width, config.media.layout);
        let stock = stock_geometry_model();
        let stock_config = stock_config();
        let stock_pressure =
            stock.media_pressure_px(stock_config.panel.width, stock_config.media.layout);

        // The delta from stock is easier to reason about than an absolute guess
        stock_config.panel.width + (pressure - stock_pressure)
    }

    fn media_pressure_px(&self, panel_width_px: i32, layout: MediaLayout) -> i32 {
        // Keep css-check aligned with the real text width reserve used by the widget builder
        let text_width_px = panel_width_px
            .saturating_sub(media_text_reserve_px(layout))
            .max(MEDIA_TEXT_WIDTH_FLOOR_PX);
        let meta_width_px = self
            .media_meta
            .outer_width_px(text_width_px)
            .max(text_width_px);

        // Buttons and artwork are the parts most likely to widen the row
        let controls_width_px = self.media_controls.outer_insets_px()
            + (self
                .media_button
                .outer_width_px(MEDIA_BUTTON_FALLBACK_WIDTH_PX)
                * 3)
            + (MEDIA_CONTROL_BUTTON_SPACING_PX * 2);
        let nav_pair_width_px =
            (self.media_nav.outer_width_px(MEDIA_NAV_FALLBACK_WIDTH_PX) * 2) + MEDIA_ROW_SPACING_PX;
        let text_row_width_px = self
            .media_art_frame
            .outer_width_px(MEDIA_ART_FRAME_FALLBACK_WIDTH_PX)
            + MEDIA_CARD_CONTENT_SPACING_PX
            + meta_width_px;
        let control_strip_width_px = self.media_control_strip.outer_insets_px()
            + nav_pair_width_px
            + MEDIA_ROW_SPACING_PX
            + controls_width_px;
        let action_nav_width_px = self.media_nav_strip.outer_insets_px() + nav_pair_width_px;
        let action_rail_width_px = self.media_action_rail.outer_insets_px()
            + controls_width_px.max(action_nav_width_px)
            + MEDIA_ACTION_RAIL_SPACING_PX;

        // Each layout spends its fixed chrome differently, so keep the width math explicit
        let card_inner_width_px = match layout {
            MediaLayout::Carousel => {
                text_row_width_px
                    + MEDIA_CARD_CONTENT_SPACING_PX
                    + controls_width_px
                    + self.media_main.outer_insets_px()
            }
            MediaLayout::Inline | MediaLayout::Stacked => {
                text_row_width_px.max(control_strip_width_px) + self.media_main.outer_insets_px()
            }
            MediaLayout::Showcase => {
                text_row_width_px
                    + MEDIA_CARD_CONTENT_SPACING_PX
                    + action_rail_width_px
                    + self.media_main.outer_insets_px()
            }
        };
        let card_outer_width_px = self.media_card.outer_width_px(card_inner_width_px);

        let external_nav_width_px = match layout {
            MediaLayout::Carousel => nav_pair_width_px + MEDIA_ROW_SPACING_PX,
            MediaLayout::Inline | MediaLayout::Stacked | MediaLayout::Showcase => 0,
        };

        // The full row also carries panel, container, stack, row, and any outer nav chrome
        self.panel.inner_insets_px()
            + self.media_container.outer_insets_px()
            + self.media_stack.outer_insets_px()
            + self.media_row.outer_insets_px()
            + card_outer_width_px
            + external_nav_width_px
    }
}

fn media_text_reserve_px(layout: MediaLayout) -> i32 {
    // Keep these reserves in lockstep with ui/media_widget/layout.rs
    match layout {
        MediaLayout::Carousel => MEDIA_CAROUSEL_TEXT_RESERVE_PX,
        MediaLayout::Inline => MEDIA_INLINE_TEXT_RESERVE_PX,
        MediaLayout::Stacked => MEDIA_STACKED_TEXT_RESERVE_PX,
        MediaLayout::Showcase => MEDIA_SHOWCASE_TEXT_RESERVE_PX,
    }
}
