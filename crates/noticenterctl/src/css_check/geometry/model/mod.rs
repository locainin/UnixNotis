//! Geometry box metrics, fixed grids, media layouts, and accumulated state

mod box_metrics;
mod constants;
mod fixed_grid;
mod media;
mod state;
mod tracking;

pub(in crate::css_check::geometry) use super::stock::baselines::{
    stock_config, stock_geometry_model,
};
pub(in crate::css_check::geometry) use box_metrics::{
    HorizontalBoxMetrics, HorizontalEdges, VerticalBoxMetrics, VerticalEdges,
};
pub(in crate::css_check::geometry) use state::{width_warning, GeometryModel};

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
