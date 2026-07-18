use unixnotis_core::{NumericParseMode, SliderWidgetConfig};

use super::SliderRefreshRequest;

#[test]
fn refresh_request_copies_every_runtime_input_from_config() {
    let config = SliderWidgetConfig {
        get_cmd: "read-custom-value".to_string(),
        min: -12.5,
        max: 240.0,
        step: 0.25,
        parse_mode: NumericParseMode::Ratio,
        ..SliderWidgetConfig::default()
    };

    let request = SliderRefreshRequest::from_config(&config);

    assert_eq!(request.command(), "read-custom-value");
    assert_eq!(request.cmd, "read-custom-value");
    assert_close(request.min, -12.5);
    assert_close(request.max, 240.0);
    assert_close(request.step, 0.25);
    assert_eq!(request.parse_mode, NumericParseMode::Ratio);
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < f64::EPSILON);
}
