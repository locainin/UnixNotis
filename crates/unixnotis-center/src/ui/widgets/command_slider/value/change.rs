//! Slider value change comparisons

pub(in super::super) fn slider_value_changed(current: f64, next: f64, step: f64) -> bool {
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

#[cfg(test)]
#[path = "tests/change.rs"]
mod tests;
