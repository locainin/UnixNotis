use super::{GeometryModel, HorizontalBoxMetrics};

impl GeometryModel {
    pub(in super::super) fn target_mut(
        &mut self,
        class_name: &str,
    ) -> Option<&mut HorizontalBoxMetrics> {
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
            ".unixnotis-media-row" => Some(&mut self.media_row),
            ".unixnotis-media-row-carousel" => Some(&mut self.media_row),
            ".unixnotis-media-row-inline" => Some(&mut self.media_row),
            ".unixnotis-media-row-stacked" => Some(&mut self.media_row),
            ".unixnotis-media-row-showcase" => Some(&mut self.media_row),
            ".unixnotis-media-main" => Some(&mut self.media_main),
            ".unixnotis-media-meta" => Some(&mut self.media_meta),
            ".unixnotis-media-nav" => Some(&mut self.media_nav),
            ".unixnotis-media-nav-strip" => Some(&mut self.media_nav_strip),
            ".unixnotis-media-card" => Some(&mut self.media_card),
            ".unixnotis-media-card-carousel" => Some(&mut self.media_card),
            ".unixnotis-media-card-inline" => Some(&mut self.media_card),
            ".unixnotis-media-card-stacked" => Some(&mut self.media_card),
            ".unixnotis-media-card-showcase" => Some(&mut self.media_card),
            ".unixnotis-media-art" => Some(&mut self.media_art),
            ".unixnotis-media-art-frame" => Some(&mut self.media_art_frame),
            ".unixnotis-media-control-strip" => Some(&mut self.media_control_strip),
            ".unixnotis-media-action-rail" => Some(&mut self.media_action_rail),
            ".unixnotis-media-controls" => Some(&mut self.media_controls),
            ".unixnotis-media-button" => Some(&mut self.media_button),
            _ => None,
        }
    }
}

pub(in super::super) fn is_tracked_class(class_name: &str) -> bool {
    // Keep selector warnings in sync with the same widget map used by the model
    matches!(
        class_name,
        ".unixnotis-panel"
            | ".unixnotis-toggle-section"
            | ".unixnotis-toggle-grid"
            | ".unixnotis-toggle"
            | ".unixnotis-stat-section"
            | ".unixnotis-stat-grid"
            | ".unixnotis-stat-card"
            | ".unixnotis-card-section"
            | ".unixnotis-card-grid"
            | ".unixnotis-info-card"
            | ".unixnotis-media-container"
            | ".unixnotis-media-stack"
            | ".unixnotis-media-stack-carousel"
            | ".unixnotis-media-stack-inline"
            | ".unixnotis-media-stack-stacked"
            | ".unixnotis-media-stack-showcase"
            | ".unixnotis-media-row"
            | ".unixnotis-media-row-carousel"
            | ".unixnotis-media-row-inline"
            | ".unixnotis-media-row-stacked"
            | ".unixnotis-media-row-showcase"
            | ".unixnotis-media-main"
            | ".unixnotis-media-meta"
            | ".unixnotis-media-nav"
            | ".unixnotis-media-nav-strip"
            | ".unixnotis-media-card"
            | ".unixnotis-media-card-carousel"
            | ".unixnotis-media-card-inline"
            | ".unixnotis-media-card-stacked"
            | ".unixnotis-media-card-showcase"
            | ".unixnotis-media-art"
            | ".unixnotis-media-art-frame"
            | ".unixnotis-media-control-strip"
            | ".unixnotis-media-action-rail"
            | ".unixnotis-media-controls"
            | ".unixnotis-media-button"
    )
}
