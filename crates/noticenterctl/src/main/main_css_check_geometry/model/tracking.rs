use super::{GeometryModel, HorizontalBoxMetrics, VerticalBoxMetrics};

impl GeometryModel {
    pub(in super::super) fn target_mut(
        &mut self,
        class_name: &str,
    ) -> Option<&mut HorizontalBoxMetrics> {
        // This table is intentionally small
        // Only selectors that own meaningful horizontal budget get modeled directly
        // Everything else is either checked against stock baselines or warned as unmodeled
        // Only selectors that map to real width-owning widgets are tracked here
        match class_name {
            ".unixnotis-panel" => Some(&mut self.panel),
            ".unixnotis-toggle-section" => Some(&mut self.toggle_section),
            ".unixnotis-toggle-grid" => Some(&mut self.toggle_grid),
            ".unixnotis-toggle" => Some(&mut self.toggle_item),
            ".unixnotis-stat-section" => Some(&mut self.stat_section),
            ".unixnotis-stat-grid" => Some(&mut self.stat_grid),
            ".unixnotis-stat-card" => Some(&mut self.stat_item),
            ".unixnotis-card-section" => Some(&mut self.card_section),
            ".unixnotis-card-grid" => Some(&mut self.card_grid),
            ".unixnotis-info-card" => Some(&mut self.card_item),
            ".unixnotis-media-container" => Some(&mut self.media_container),
            ".unixnotis-media-stack" => Some(&mut self.media_stack),
            ".unixnotis-media-stack-carousel" => Some(&mut self.media_stack),
            ".unixnotis-media-stack-inline" => Some(&mut self.media_stack),
            ".unixnotis-media-stack-stacked" => Some(&mut self.media_stack),
            ".unixnotis-media-stack-showcase" => Some(&mut self.media_stack),
            ".unixnotis-media-stack-player" => Some(&mut self.media_stack),
            ".unixnotis-media-row" => Some(&mut self.media_row),
            ".unixnotis-media-row-carousel" => Some(&mut self.media_row),
            ".unixnotis-media-row-inline" => Some(&mut self.media_row),
            ".unixnotis-media-row-stacked" => Some(&mut self.media_row),
            ".unixnotis-media-row-showcase" => Some(&mut self.media_row),
            ".unixnotis-media-row-player" => Some(&mut self.media_row),
            ".unixnotis-media-header" => Some(&mut self.media_header),
            ".unixnotis-media-body" => Some(&mut self.media_body),
            ".unixnotis-media-text" => Some(&mut self.media_text),
            ".unixnotis-media-main" => Some(&mut self.media_main),
            ".unixnotis-media-meta" => Some(&mut self.media_meta),
            ".unixnotis-media-nav" => Some(&mut self.media_nav),
            ".unixnotis-media-nav-prev" => Some(&mut self.media_nav),
            ".unixnotis-media-nav-next" => Some(&mut self.media_nav),
            ".unixnotis-media-nav-strip" => Some(&mut self.media_nav_strip),
            ".unixnotis-media-card" => Some(&mut self.media_card),
            ".unixnotis-media-card-carousel" => Some(&mut self.media_card),
            ".unixnotis-media-card-inline" => Some(&mut self.media_card),
            ".unixnotis-media-card-stacked" => Some(&mut self.media_card),
            ".unixnotis-media-card-showcase" => Some(&mut self.media_card),
            ".unixnotis-media-card-player" => Some(&mut self.media_card),
            ".unixnotis-media-art" => Some(&mut self.media_art),
            ".unixnotis-media-art-frame" => Some(&mut self.media_art_frame),
            ".unixnotis-media-control-strip" => Some(&mut self.media_control_strip),
            ".unixnotis-media-action-rail" => Some(&mut self.media_action_rail),
            ".unixnotis-media-controls" => Some(&mut self.media_controls),
            ".unixnotis-media-button" => Some(&mut self.media_button),
            ".unixnotis-media-button-prev" => Some(&mut self.media_button),
            ".unixnotis-media-button-play" => Some(&mut self.media_button),
            ".unixnotis-media-button-next" => Some(&mut self.media_button),
            _ => None,
        }
    }

    pub(in super::super) fn target_vertical_mut(
        &mut self,
        class_name: &str,
    ) -> Option<&mut VerticalBoxMetrics> {
        match class_name {
            ".unixnotis-media-header" => Some(&mut self.media_vertical.header),
            ".unixnotis-media-body" => Some(&mut self.media_vertical.body),
            ".unixnotis-media-text" => Some(&mut self.media_vertical.text),
            ".unixnotis-media-main" => Some(&mut self.media_vertical.main),
            ".unixnotis-media-meta" => Some(&mut self.media_vertical.meta),
            ".unixnotis-media-source" => Some(&mut self.media_vertical.source),
            ".unixnotis-media-position" => Some(&mut self.media_vertical.position),
            ".unixnotis-media-title" => Some(&mut self.media_vertical.title),
            ".unixnotis-media-artist" => Some(&mut self.media_vertical.artist),
            ".unixnotis-media-nav" => Some(&mut self.media_vertical.nav),
            ".unixnotis-media-nav-prev" => Some(&mut self.media_vertical.nav),
            ".unixnotis-media-nav-next" => Some(&mut self.media_vertical.nav),
            ".unixnotis-media-nav-strip" => Some(&mut self.media_vertical.nav_strip),
            ".unixnotis-media-card" => Some(&mut self.media_vertical.card),
            ".unixnotis-media-card-carousel" => Some(&mut self.media_vertical.card),
            ".unixnotis-media-card-inline" => Some(&mut self.media_vertical.card),
            ".unixnotis-media-card-stacked" => Some(&mut self.media_vertical.card),
            ".unixnotis-media-card-showcase" => Some(&mut self.media_vertical.card),
            ".unixnotis-media-card-player" => Some(&mut self.media_vertical.card),
            ".unixnotis-media-art-frame" => Some(&mut self.media_vertical.art_frame),
            ".unixnotis-media-control-strip" => Some(&mut self.media_vertical.control_strip),
            ".unixnotis-media-action-rail" => Some(&mut self.media_vertical.action_rail),
            ".unixnotis-media-controls" => Some(&mut self.media_vertical.controls),
            ".unixnotis-media-button" => Some(&mut self.media_vertical.button),
            ".unixnotis-media-button-prev" => Some(&mut self.media_vertical.button),
            ".unixnotis-media-button-play" => Some(&mut self.media_vertical.button),
            ".unixnotis-media-button-next" => Some(&mut self.media_vertical.button),
            _ => None,
        }
    }
}
