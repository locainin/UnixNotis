//! Collapsed notification stack state

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StackLayerVisibility {
    pub(super) middle: bool,
    pub(super) back: bool,
}

pub(super) const fn layer_visibility(stack_depth: u8) -> StackLayerVisibility {
    // Depth one uses the back slot because its card starts without overlap
    StackLayerVisibility {
        middle: stack_depth >= 2,
        back: stack_depth >= 1,
    }
}

#[cfg(test)]
#[path = "tests/stack.rs"]
mod tests;
