use std::collections::HashMap;

use super::{GeometryModel, HorizontalBoxMetrics, VerticalBoxMetrics};

#[test]
fn horizontal_box_metrics_use_larger_content_width_and_all_horizontal_edges() {
    let custom_properties = HashMap::new();
    let mut metrics = HorizontalBoxMetrics::default();

    metrics.apply_property("width", "100px", &custom_properties);
    metrics.apply_property("min-width", "120px", &custom_properties);
    metrics.apply_property("margin", "2px 4px 6px 8px", &custom_properties);
    metrics.apply_property("padding-left", "3px", &custom_properties);
    metrics.apply_property("padding-right", "5px", &custom_properties);
    metrics.apply_property("border-width", "1px 2px", &custom_properties);

    assert_eq!(metrics.outer_width_px(10), 144);
    assert_eq!(metrics.outer_insets_px(), 24);
    assert_eq!(metrics.inner_insets_px(), 12);
}

#[test]
fn vertical_box_metrics_use_larger_content_height_and_vertical_edges() {
    let custom_properties = HashMap::new();
    let mut metrics = VerticalBoxMetrics::default();

    metrics.apply_property("height", "20px", &custom_properties);
    metrics.apply_property("min-height", "28px", &custom_properties);
    metrics.apply_property("margin", "1px 2px 3px 4px", &custom_properties);
    metrics.apply_property("padding", "2px 9px", &custom_properties);
    metrics.apply_property("border-top-width", "4px", &custom_properties);
    metrics.apply_property("border-bottom-width", "6px", &custom_properties);

    assert_eq!(metrics.outer_height_px(10), 46);
}

#[test]
fn media_aliases_map_to_shared_width_and_height_targets() {
    let mut model = GeometryModel::default();

    model
        .target_mut(".unixnotis-media-button-prev")
        .expect("media button alias")
        .apply_property("min-width", "31px", &HashMap::new());
    let button = model
        .target_mut(".unixnotis-media-button")
        .expect("media button target");
    assert_eq!(button.outer_width_px(0), 31);

    model
        .target_vertical_mut(".unixnotis-media-card-carousel")
        .expect("media card alias")
        .apply_property("min-height", "44px", &HashMap::new());
    let card = model
        .target_vertical_mut(".unixnotis-media-card")
        .expect("media card target");
    assert_eq!(card.outer_height_px(0), 44);
}

#[test]
fn unknown_classes_do_not_return_geometry_targets() {
    let mut model = GeometryModel::default();

    assert!(model.target_mut(".unixnotis-not-a-widget").is_none());
    assert!(model
        .target_vertical_mut(".unixnotis-not-a-widget")
        .is_none());
}
