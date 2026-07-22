//! Shared GTK motion-policy operations

pub(super) fn apply_revealer_preference(
    revealer: &gtk::Revealer,
    standard_duration_ms: u32,
    reduced_motion: bool,
) {
    revealer.set_transition_duration(if reduced_motion {
        0
    } else {
        standard_duration_ms
    });

    if let Some([edge, target]) = immediate_reveal_edges(
        reduced_motion,
        revealer.is_child_revealed(),
        revealer.reveals_child(),
    ) {
        // Reapplying the target through an immediate edge finishes an animation already in flight
        revealer.set_reveal_child(edge);
        revealer.set_reveal_child(target);
    }
}

const fn immediate_reveal_edges(
    reduced_motion: bool,
    child_revealed: bool,
    target: bool,
) -> Option<[bool; 2]> {
    if reduced_motion && child_revealed != target {
        Some([!target, target])
    } else {
        None
    }
}

#[cfg(test)]
#[path = "tests/motion.rs"]
mod tests;
