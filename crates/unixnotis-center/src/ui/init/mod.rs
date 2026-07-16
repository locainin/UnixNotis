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
#[path = "tests/actions.rs"]
mod actions_tests;
#[cfg(test)]
#[path = "tests/autoclose.rs"]
mod autoclose_tests;
#[cfg(test)]
#[path = "tests/builders.rs"]
mod builders_tests;
#[cfg(test)]
#[path = "tests/constructor.rs"]
mod constructor_tests;
#[cfg(test)]
#[path = "tests/keyboard.rs"]
mod keyboard_tests;
#[cfg(test)]
#[path = "tests/search.rs"]
mod search_tests;
#[cfg(test)]
#[path = "tests/timing.rs"]
mod timing_tests;
