//! Center UI startup wiring

// Focused child modules keep construction separate from long-lived UI behavior
mod actions;
mod autoclose;
mod builders;
mod constructor;
mod keyboard;
mod search;
mod timing;

#[cfg(test)]
#[path = "init/tests/keyboard.rs"]
mod keyboard_tests;
