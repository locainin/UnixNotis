//! Center startup wiring sourced outside the UI folder

// This module stays mounted under `ui` so startup code can wire private UI parts
// Files live under `src/init` to keep constructor logic out of `src/ui`
mod actions;
mod autoclose;
mod builders;
mod constructor;
mod keyboard;
mod search;
mod timing;

#[cfg(test)]
#[path = "tests/keyboard.rs"]
mod keyboard_tests;
