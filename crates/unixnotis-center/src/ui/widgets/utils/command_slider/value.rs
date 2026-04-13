//! Slider value formatting and refresh comparison helpers

pub(super) fn slider_value_changed(current: f64, next: f64, step: f64) -> bool {
    // Treat values inside half a step as unchanged for UI refresh decisions
    (current - next).abs() > slider_value_tolerance(step)
}

pub(super) fn slider_value_tolerance(step: f64) -> f64 {
    // Broken or missing step values fall back to a tiny fixed tolerance
    if !step.is_finite() || step <= 0.0 {
        return 1e-6;
    }
    (step * 0.5).max(1e-6)
}

pub(super) fn format_command_value(value: f64, step: f64) -> String {
    // Match command precision to slider granularity so fractional sliders stay correct
    let precision = slider_step_precision(step);
    let formatted = format!("{value:.precision$}");
    trim_decimal_suffix(formatted)
}

fn slider_step_precision(step: f64) -> usize {
    if !step.is_finite() || step <= 0.0 {
        return 0;
    }

    // Stop once the step looks like a whole number at this precision
    for precision in 0..=6 {
        let factor = 10f64.powi(precision as i32);
        let scaled = step * factor;
        if (scaled.round() - scaled).abs() <= 1e-9 {
            return precision;
        }
    }

    6
}

fn trim_decimal_suffix(mut text: String) -> String {
    // Drop trailing zeroes so commands get `12.5` instead of `12.500`
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}
