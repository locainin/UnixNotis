use unixnotis_core::Config;

use super::constants::{
    CARD_FALLBACK_CONTENT_WIDTH_PX, CARD_GRID_COLUMNS, CARD_GRID_SPACING_PX,
    STAT_FALLBACK_CONTENT_WIDTH_PX, STAT_GRID_COLUMNS, STAT_GRID_SPACING_PX,
    TOGGLE_FALLBACK_CONTENT_WIDTH_PX, TOGGLE_GRID_COLUMNS, TOGGLE_GRID_SPACING_PX,
};
use super::{
    stock_config, stock_geometry_model, width_warning, GeometryModel, HorizontalBoxMetrics,
};

// Toggle, stat, and card warnings all use the same fixed-column width math
impl GeometryModel {
    pub(super) fn toggle_grid_warning(&self, config: &Config) -> Option<String> {
        let required_panel_width_px = self.toggle_required_panel_width_px(config)?;
        width_warning(
            "toggle grid",
            required_panel_width_px,
            config.panel.width,
            "GTK natural width can still widen the panel when fixed columns ask for more room",
        )
    }

    pub(super) fn stat_grid_warning(&self, config: &Config) -> Option<String> {
        let required_panel_width_px = self.stat_required_panel_width_px(config)?;
        width_warning(
            "stat grid",
            required_panel_width_px,
            config.panel.width,
            "GTK natural width can still widen the panel when two-column cards ask for more room",
        )
    }

    pub(super) fn card_grid_warning(&self, config: &Config) -> Option<String> {
        let required_panel_width_px = self.card_required_panel_width_px(config)?;
        width_warning(
            "card grid",
            required_panel_width_px,
            config.panel.width,
            "GTK natural width can still widen the panel when two-column cards ask for more room",
        )
    }

    fn toggle_required_panel_width_px(&self, config: &Config) -> Option<i32> {
        // Only enabled widgets can claim columns in the live grid
        let columns = config
            .widgets
            .toggles
            .iter()
            .filter(|widget| widget.enabled)
            .count()
            .min(TOGGLE_GRID_COLUMNS);
        if columns == 0 {
            return None;
        }

        // Panel padding, section padding, grid padding, item width, and spacing all add pressure
        let pressure = self.fixed_grid_pressure_px(
            columns,
            TOGGLE_GRID_SPACING_PX,
            self.toggle_section,
            self.toggle_grid,
            self.toggle_item
                .outer_width_px(TOGGLE_FALLBACK_CONTENT_WIDTH_PX),
        );

        // Compare against the shipped theme so stock CSS stays quiet
        let stock = stock_geometry_model();
        let stock_config = stock_config();
        let stock_columns = stock_config
            .widgets
            .toggles
            .iter()
            .filter(|widget| widget.enabled)
            .count()
            .min(TOGGLE_GRID_COLUMNS);
        let stock_pressure = stock.fixed_grid_pressure_px(
            stock_columns,
            TOGGLE_GRID_SPACING_PX,
            stock.toggle_section,
            stock.toggle_grid,
            stock
                .toggle_item
                .outer_width_px(TOGGLE_FALLBACK_CONTENT_WIDTH_PX),
        );

        Some(stock_config.panel.width + (pressure - stock_pressure))
    }

    fn stat_required_panel_width_px(&self, config: &Config) -> Option<i32> {
        // Stats render as a fixed two-column grid, so enabled items decide the live column count
        let columns = config
            .widgets
            .stats
            .iter()
            .filter(|widget| widget.enabled)
            .count()
            .min(STAT_GRID_COLUMNS);
        if columns == 0 {
            return None;
        }

        let pressure = self.fixed_grid_pressure_px(
            columns,
            STAT_GRID_SPACING_PX,
            self.stat_section,
            self.stat_grid,
            self.stat_item
                .outer_width_px(STAT_FALLBACK_CONTENT_WIDTH_PX),
        );

        let stock = stock_geometry_model();
        let stock_config = stock_config();
        let stock_columns = stock_config
            .widgets
            .stats
            .iter()
            .filter(|widget| widget.enabled)
            .count()
            .min(STAT_GRID_COLUMNS);
        let stock_pressure = stock.fixed_grid_pressure_px(
            stock_columns,
            STAT_GRID_SPACING_PX,
            stock.stat_section,
            stock.stat_grid,
            stock
                .stat_item
                .outer_width_px(STAT_FALLBACK_CONTENT_WIDTH_PX),
        );

        Some(stock_config.panel.width + (pressure - stock_pressure))
    }

    fn card_required_panel_width_px(&self, config: &Config) -> Option<i32> {
        // Cards use the same fixed-column pattern as stats, with wider default content
        let columns = config
            .widgets
            .cards
            .iter()
            .filter(|widget| widget.enabled)
            .count()
            .min(CARD_GRID_COLUMNS);
        if columns == 0 {
            return None;
        }

        let pressure = self.fixed_grid_pressure_px(
            columns,
            CARD_GRID_SPACING_PX,
            self.card_section,
            self.card_grid,
            self.card_item
                .outer_width_px(CARD_FALLBACK_CONTENT_WIDTH_PX),
        );

        let stock = stock_geometry_model();
        let stock_config = stock_config();
        let stock_columns = stock_config
            .widgets
            .cards
            .iter()
            .filter(|widget| widget.enabled)
            .count()
            .min(CARD_GRID_COLUMNS);
        let stock_pressure = stock.fixed_grid_pressure_px(
            stock_columns,
            CARD_GRID_SPACING_PX,
            stock.card_section,
            stock.card_grid,
            stock
                .card_item
                .outer_width_px(CARD_FALLBACK_CONTENT_WIDTH_PX),
        );

        Some(stock_config.panel.width + (pressure - stock_pressure))
    }

    fn fixed_grid_pressure_px(
        &self,
        columns: usize,
        spacing_px: i32,
        section: HorizontalBoxMetrics,
        grid: HorizontalBoxMetrics,
        item_outer_width_px: i32,
    ) -> i32 {
        // The panel loses width to its own chrome first
        self.panel.inner_insets_px()
            // Section and grid wrappers also eat width before items are laid out
            + section.outer_insets_px()
            + grid.outer_insets_px()
            // Each column brings its own outer box width
            + (columns as i32 * item_outer_width_px)
            // Spacing only exists between neighbors, never after the last item
            + ((columns.saturating_sub(1)) as i32 * spacing_px)
    }
}
