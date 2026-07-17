//! Center UI startup wiring

// Startup owns construction while long-lived panel behavior stays with the panel domain
mod builders;
mod constructor;

#[cfg(test)]
#[path = "tests/builders.rs"]
mod builders_tests;
#[cfg(test)]
#[path = "tests/constructor.rs"]
mod constructor_tests;
