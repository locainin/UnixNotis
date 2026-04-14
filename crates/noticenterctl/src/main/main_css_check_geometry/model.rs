//! Geometry model and width-pressure math for css-check

use unixnotis_core::Config;

use super::stock::{stock_config, stock_geometry_model};

// Keep the geometry model split by job so width math changes stay easy to trace
#[path = "model/box_metrics.rs"]
mod box_metrics;
#[path = "model/constants.rs"]
mod constants;
#[path = "model/fixed_grid.rs"]
mod fixed_grid;
#[path = "model/media.rs"]
mod media;
#[path = "model/tracking.rs"]
mod tracking;

pub(super) use self::box_metrics::{HorizontalBoxMetrics, HorizontalEdges};
use self::constants::WIDTH_WARNING_TOLERANCE_PX;
pub(super) use self::tracking::is_tracked_class;

#[derive(Default)]
pub(super) struct GeometryModel {
    // Panel chrome is the first width budget every child must fit inside
    panel: HorizontalBoxMetrics,
    // Toggle widths are tracked as section, grid, and item layers
    toggle_section: HorizontalBoxMetrics,
    toggle_grid: HorizontalBoxMetrics,
    toggle_item: HorizontalBoxMetrics,
    // Stat widths follow the same pattern with a different grid size
    stat_section: HorizontalBoxMetrics,
    stat_grid: HorizontalBoxMetrics,
    stat_item: HorizontalBoxMetrics,
    // Info cards share the fixed-grid math too
    card_section: HorizontalBoxMetrics,
    card_grid: HorizontalBoxMetrics,
    card_item: HorizontalBoxMetrics,
    // Media carries more moving parts, so each width-owning node is tracked on its own
    media_container: HorizontalBoxMetrics,
    media_stack: HorizontalBoxMetrics,
    media_row: HorizontalBoxMetrics,
    media_main: HorizontalBoxMetrics,
    media_meta: HorizontalBoxMetrics,
    media_nav: HorizontalBoxMetrics,
    media_nav_strip: HorizontalBoxMetrics,
    media_card: HorizontalBoxMetrics,
    media_art: HorizontalBoxMetrics,
    media_art_frame: HorizontalBoxMetrics,
    media_control_strip: HorizontalBoxMetrics,
    media_action_rail: HorizontalBoxMetrics,
    media_controls: HorizontalBoxMetrics,
    media_button: HorizontalBoxMetrics,
}

impl GeometryModel {
    pub(super) fn finalize_warnings(&self, config: &Config) -> Vec<String> {
        let mut warnings = Vec::new();

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
        if let Some(warning) = self.media_art_target_warning() {
            warnings.push(warning);
        }

        warnings
    }
}

fn width_warning(
    label: &str,
    required_panel_width_px: i32,
    panel_width_px: i32,
    natural_width_note: &str,
) -> Option<String> {
    // Small rounding drift should not become a warning
    if required_panel_width_px <= panel_width_px + WIDTH_WARNING_TOLERANCE_PX {
        return None;
    }

    Some(format!(
        "{label} looks like it needs about {required_panel_width_px}px of panel width, but [panel].width={panel_width_px}; {natural_width_note}"
    ))
}
