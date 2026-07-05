//! Stock theme helpers for geometry lint

pub(super) mod baselines;
pub(super) mod classes;
mod size_rules;

pub(super) use self::size_rules::{
    normalized_horizontal_size_rules, should_warn_for_unmodeled_known_class,
};

#[cfg(test)]
mod tests;
