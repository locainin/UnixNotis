//! Bounded polygon construction and hit testing

use gtk::gsk;
use unixnotis_core::CutCorners;

#[derive(Clone, Copy)]
struct NormalizedCorners {
    top_left: f32,
    top_right: f32,
    bottom_right: f32,
    bottom_left: f32,
}

impl NormalizedCorners {
    fn new(width: f32, height: f32, corners: CutCorners) -> Self {
        // Half-edge limits stop neighboring diagonal cuts from crossing
        let limit = (width.max(0.0) / 2.0).min(height.max(0.0) / 2.0);
        Self {
            top_left: f32::from(corners.top_left).min(limit),
            top_right: f32::from(corners.top_right).min(limit),
            bottom_right: f32::from(corners.bottom_right).min(limit),
            bottom_left: f32::from(corners.bottom_left).min(limit),
        }
    }
}

pub(super) fn build_path(width: f32, height: f32, corners: CutCorners) -> gsk::Path {
    let width = width.max(0.0);
    let height = height.max(0.0);
    let corners = NormalizedCorners::new(width, height, corners);
    let path = gsk::PathBuilder::new();

    // Clockwise points form one convex plate with a diagonal at every active corner
    path.move_to(corners.top_left, 0.0);
    path.line_to(width - corners.top_right, 0.0);
    path.line_to(width, corners.top_right);
    path.line_to(width, height - corners.bottom_right);
    path.line_to(width - corners.bottom_right, height);
    path.line_to(corners.bottom_left, height);
    path.line_to(0.0, height - corners.bottom_left);
    path.line_to(0.0, corners.top_left);
    path.close();
    path.to_path()
}

pub(super) fn contains_point(width: f32, height: f32, corners: CutCorners, x: f64, y: f64) -> bool {
    let x = x as f32;
    let y = y as f32;
    if x < 0.0 || y < 0.0 || x >= width || y >= height {
        // GTK hit testing excludes the far allocation edge
        return false;
    }

    let corners = NormalizedCorners::new(width, height, corners);
    x + y >= corners.top_left
        && (width - x) + y >= corners.top_right
        && (width - x) + (height - y) >= corners.bottom_right
        && x + (height - y) >= corners.bottom_left
}
