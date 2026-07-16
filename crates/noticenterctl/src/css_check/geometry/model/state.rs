//! Collected geometry state and final panel budget warnings

use unixnotis_core::Config;

use super::box_metrics::HorizontalBoxMetrics;
use super::constants::WIDTH_WARNING_TOLERANCE_PX;
use super::media;

#[derive(Default)]
pub(in crate::css_check::geometry) struct GeometryModel {
    // Panel chrome is the first width budget every child must fit inside
    pub(super) panel: HorizontalBoxMetrics,
    // Toggle widths are tracked as section, grid, and item layers
    pub(super) toggle_section: HorizontalBoxMetrics,
    pub(super) toggle_grid: HorizontalBoxMetrics,
    pub(super) toggle_item: HorizontalBoxMetrics,
    // Stat widths follow the same pattern with a different grid size
    pub(super) stat_section: HorizontalBoxMetrics,
    pub(super) stat_grid: HorizontalBoxMetrics,
    pub(super) stat_item: HorizontalBoxMetrics,
    // Info cards share the fixed-grid math too
    pub(super) card_section: HorizontalBoxMetrics,
    pub(super) card_grid: HorizontalBoxMetrics,
    pub(super) card_item: HorizontalBoxMetrics,
    // Media carries more moving parts, so each width-owning node is tracked on its own
    pub(super) media_container: HorizontalBoxMetrics,
    pub(super) media_stack: HorizontalBoxMetrics,
    pub(super) media_row: HorizontalBoxMetrics,
    pub(super) media_header: HorizontalBoxMetrics,
    pub(super) media_body: HorizontalBoxMetrics,
    pub(super) media_text: HorizontalBoxMetrics,
    pub(super) media_main: HorizontalBoxMetrics,
    pub(super) media_meta: HorizontalBoxMetrics,
    pub(super) media_nav: HorizontalBoxMetrics,
    pub(super) media_nav_strip: HorizontalBoxMetrics,
    pub(super) media_card: HorizontalBoxMetrics,
    pub(super) media_art: HorizontalBoxMetrics,
    pub(super) media_art_frame: HorizontalBoxMetrics,
    pub(super) media_control_strip: HorizontalBoxMetrics,
    pub(super) media_action_rail: HorizontalBoxMetrics,
    pub(super) media_controls: HorizontalBoxMetrics,
    pub(super) media_button: HorizontalBoxMetrics,
    // Media height feasibility uses a separate vertical box model to avoid disturbing width math
    pub(super) media_vertical: media::MediaVerticalModel,
}

impl GeometryModel {
    pub(in crate::css_check::geometry) fn finalize_warnings(&self, config: &Config) -> Vec<String> {
        let mut warnings = Vec::new();

        // The file-level scan only gathers numbers
        // The actual panel budget check happens here once every selector has had a chance
        // to update the model
        // Each section is checked on its own so the warning stays easy to read
        if let Some(warning) = self.toggle_grid_warning(config) {
            warnings.push(warning);
        }
        if let Some(warning) = self.stat_grid_warning(config) {
            warnings.push(warning);
        }
        if let Some(warning) = self.card_grid_warning(config) {
            warnings.push(warning);
        }
        if let Some(warning) = self.media_width_warning(config) {
            warnings.push(warning);
        }
        if let Some(warning) = self.media_height_warning(config) {
            warnings.push(warning);
        }
        if let Some(warning) = self.media_art_target_warning() {
            warnings.push(warning);
        }

        warnings
    }
}

pub(in crate::css_check::geometry) fn width_warning(
    label: &str,
    required_panel_width_px: i32,
    panel_width_px: i32,
    natural_width_note: &str,
) -> Option<String> {
    // Small rounding drift should not become a warning
    if required_panel_width_px <= panel_width_px + WIDTH_WARNING_TOLERANCE_PX {
        return None;
    }

    // The message keeps both numbers visible so a theme author can see whether the issue is
    // a tiny overshoot or a layout that is far outside the configured panel budget
    Some(format!(
        "{label} looks like it needs about {required_panel_width_px}px of panel width, but [panel].width={panel_width_px}; {natural_width_note}"
    ))
}
