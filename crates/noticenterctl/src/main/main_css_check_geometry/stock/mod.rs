//! Stock theme helpers for geometry lint

#[path = "baselines.rs"]
pub(super) mod baselines;
#[path = "classes.rs"]
pub(super) mod classes;
mod size_rules;

pub(super) use self::size_rules::{
    normalized_horizontal_size_rules, should_warn_for_unmodeled_known_class,
};
