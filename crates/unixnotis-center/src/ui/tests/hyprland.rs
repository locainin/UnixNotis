use super::{monitor_matches_output, parse_reserved};

#[test]
fn parse_reserved_array_order() {
    // [top, right, bottom, left] -> Margins { top, right, bottom, left }
    let value = serde_json::json!([10, 20, 30, 40]);
    let margins = parse_reserved(&value).expect("reserved margins");
    assert_eq!(margins.top, 10);
    assert_eq!(margins.right, 20);
    assert_eq!(margins.bottom, 30);
    assert_eq!(margins.left, 40);
}

#[test]
fn monitor_matches_output_supports_connector_name() {
    let monitor = serde_json::json!({
        "name": "DP-1",
        "description": "LG Display"
    });
    assert!(monitor_matches_output(&monitor, "dp-1"));
}

#[test]
fn monitor_matches_output_supports_model_fallback() {
    let monitor = serde_json::json!({
        "name": "eDP-1",
        "model": "Laptop Screen"
    });
    assert!(monitor_matches_output(&monitor, "laptop screen"));
}
