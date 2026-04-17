use super::super::parse::CssCustomProperties;

// Geometry lint only needs left and right edges for width math
#[derive(Clone, Copy, Default)]
pub(in super::super) struct HorizontalEdges {
    pub(in super::super) left: f32,
    pub(in super::super) right: f32,
}

impl HorizontalEdges {
    pub(in super::super) fn total_px(self) -> i32 {
        // Rounding once keeps tiny float noise out of the warning math
        (self.left + self.right).round() as i32
    }
}

// Media height lint only needs top and bottom edges for vertical pressure math
#[derive(Clone, Copy, Default)]
pub(in super::super) struct VerticalEdges {
    pub(in super::super) top: f32,
    pub(in super::super) bottom: f32,
}

impl VerticalEdges {
    pub(in super::super) fn total_px(self) -> i32 {
        // One rounding pass keeps calc noise out of the final warning math
        (self.top + self.bottom).round() as i32
    }
}

#[derive(Clone, Copy, Default)]
pub(in super::super) struct HorizontalBoxMetrics {
    // Width and min-width are tracked separately so GTK fallback rules stay intact
    width: Option<f32>,
    min_width: Option<f32>,
    margin: HorizontalEdges,
    padding: HorizontalEdges,
    border: HorizontalEdges,
}

impl HorizontalBoxMetrics {
    pub(in super::super) fn apply_property(
        &mut self,
        name: &str,
        value: &str,
        custom_properties: &CssCustomProperties,
    ) {
        // Only horizontal size inputs matter for this lint pass
        match name {
            "width" => {
                self.width = super::super::parse::parse_single_length(value, custom_properties)
            }
            "min-width" => {
                self.min_width = super::super::parse::parse_single_length(value, custom_properties)
            }
            "margin" => {
                // Shorthand values can set both left and right in one pass
                if let Some(edges) = super::super::parse::parse_box_edges(value, custom_properties)
                {
                    self.margin = edges;
                }
            }
            "margin-left" => {
                super::super::parse::set_edge(&mut self.margin.left, value, custom_properties)
            }
            "margin-right" => {
                super::super::parse::set_edge(&mut self.margin.right, value, custom_properties)
            }
            "padding" => {
                // Padding still eats panel width even when child content stays the same
                if let Some(edges) = super::super::parse::parse_box_edges(value, custom_properties)
                {
                    self.padding = edges;
                }
            }
            "padding-left" => {
                super::super::parse::set_edge(&mut self.padding.left, value, custom_properties)
            }
            "padding-right" => {
                super::super::parse::set_edge(&mut self.padding.right, value, custom_properties)
            }
            "border" | "border-width" => {
                if let Some(edges) = super::super::parse::parse_box_edges(value, custom_properties)
                {
                    self.border = edges;
                }
            }
            "border-left" | "border-left-width" => {
                super::super::parse::set_edge(&mut self.border.left, value, custom_properties)
            }
            "border-right" | "border-right-width" => {
                super::super::parse::set_edge(&mut self.border.right, value, custom_properties)
            }
            _ => {}
        }
    }

    pub(in super::super) fn outer_width_px(self, fallback_px: i32) -> i32 {
        self.content_width_px(fallback_px)
            + self.margin.total_px()
            + self.padding.total_px()
            + self.border.total_px()
    }

    pub(in super::super) fn outer_insets_px(self) -> i32 {
        // Outer insets affect the parent width budget directly
        self.margin.total_px() + self.padding.total_px() + self.border.total_px()
    }

    pub(in super::super) fn inner_insets_px(self) -> i32 {
        // Panel padding and borders shrink the width left for child widgets
        self.padding.total_px() + self.border.total_px()
    }

    fn content_width_px(self, fallback_px: i32) -> i32 {
        // GTK can honor either width or min-width, so keep the larger one
        match (self.width, self.min_width) {
            (Some(width), Some(min_width)) => width.max(min_width).round() as i32,
            (Some(width), None) => width.round() as i32,
            (None, Some(min_width)) => min_width.round() as i32,
            (None, None) => fallback_px,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(in super::super) struct VerticalBoxMetrics {
    // Height and min-height are tracked separately for the same GTK sizing reason as width
    height: Option<f32>,
    min_height: Option<f32>,
    margin: VerticalEdges,
    padding: VerticalEdges,
    border: VerticalEdges,
}

impl VerticalBoxMetrics {
    pub(in super::super) fn apply_property(
        &mut self,
        name: &str,
        value: &str,
        custom_properties: &CssCustomProperties,
    ) {
        match name {
            "height" => {
                self.height = super::super::parse::parse_single_length(value, custom_properties)
            }
            "min-height" => {
                self.min_height = super::super::parse::parse_single_length(value, custom_properties)
            }
            "margin" => {
                if let Some(edges) =
                    super::super::parse::parse_box_vertical_edges(value, custom_properties)
                {
                    self.margin = edges;
                }
            }
            "margin-top" => {
                super::super::parse::set_edge(&mut self.margin.top, value, custom_properties)
            }
            "margin-bottom" => {
                super::super::parse::set_edge(&mut self.margin.bottom, value, custom_properties)
            }
            "padding" => {
                if let Some(edges) =
                    super::super::parse::parse_box_vertical_edges(value, custom_properties)
                {
                    self.padding = edges;
                }
            }
            "padding-top" => {
                super::super::parse::set_edge(&mut self.padding.top, value, custom_properties)
            }
            "padding-bottom" => {
                super::super::parse::set_edge(&mut self.padding.bottom, value, custom_properties)
            }
            "border" | "border-width" => {
                if let Some(edges) =
                    super::super::parse::parse_box_vertical_edges(value, custom_properties)
                {
                    self.border = edges;
                }
            }
            "border-top" | "border-top-width" => {
                super::super::parse::set_edge(&mut self.border.top, value, custom_properties)
            }
            "border-bottom" | "border-bottom-width" => {
                super::super::parse::set_edge(&mut self.border.bottom, value, custom_properties)
            }
            _ => {}
        }
    }

    pub(in super::super) fn outer_height_px(self, fallback_px: i32) -> i32 {
        self.content_height_px(fallback_px)
            + self.margin.total_px()
            + self.padding.total_px()
            + self.border.total_px()
    }

    fn content_height_px(self, fallback_px: i32) -> i32 {
        match (self.height, self.min_height) {
            (Some(height), Some(min_height)) => height.max(min_height).round() as i32,
            (Some(height), None) => height.round() as i32,
            (None, Some(min_height)) => min_height.round() as i32,
            (None, None) => fallback_px,
        }
    }
}
