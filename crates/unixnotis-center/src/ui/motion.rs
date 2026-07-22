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

    if !reduced_motion || revealer.is_child_revealed() == revealer.reveals_child() {
        return;
    }

    // Reapplying the target through an immediate edge finishes an animation already in flight
    let target = revealer.reveals_child();
    revealer.set_reveal_child(!target);
    revealer.set_reveal_child(target);
}

#[cfg(test)]
#[path = "tests/motion.rs"]
mod tests;
