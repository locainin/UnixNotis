use super::super::VerticalBoxMetrics;

#[derive(Default)]
pub(in super::super) struct MediaVerticalModel {
    // Card-level boxes own the outer shell stack
    pub(in super::super) card: VerticalBoxMetrics,
    pub(in super::super) header: VerticalBoxMetrics,
    pub(in super::super) body: VerticalBoxMetrics,
    pub(in super::super) text: VerticalBoxMetrics,
    pub(in super::super) main: VerticalBoxMetrics,
    // Text rows are tracked separately so css can stretch them unevenly
    pub(in super::super) meta: VerticalBoxMetrics,
    pub(in super::super) source: VerticalBoxMetrics,
    pub(in super::super) position: VerticalBoxMetrics,
    pub(in super::super) title: VerticalBoxMetrics,
    pub(in super::super) artist: VerticalBoxMetrics,
    // Nav and control boxes need their own vertical state for bottom and side rails
    pub(in super::super) nav: VerticalBoxMetrics,
    pub(in super::super) nav_strip: VerticalBoxMetrics,
    pub(in super::super) art_frame: VerticalBoxMetrics,
    pub(in super::super) control_strip: VerticalBoxMetrics,
    pub(in super::super) action_rail: VerticalBoxMetrics,
    pub(in super::super) controls: VerticalBoxMetrics,
    pub(in super::super) button: VerticalBoxMetrics,
}
