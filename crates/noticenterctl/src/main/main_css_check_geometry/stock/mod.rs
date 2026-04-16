//! Stock theme helpers for geometry lint

use std::collections::HashMap;

#[path = "baselines.rs"]
pub(super) mod baselines;
#[path = "classes.rs"]
pub(super) mod classes;

pub(super) fn should_warn_for_unmodeled_known_class(
    class_name: &str,
    properties: &[(String, String)],
) -> bool {
    // This is the quiet-stock / loud-custom split for real UnixNotis hooks that do not have
    // direct width math yet
    // Any known live class that carries custom size rules should stay visible once it leaves
    // the shipped stock baseline
    !stock_matches_horizontal_size_rules(class_name, properties)
}

fn stock_matches_horizontal_size_rules(class_name: &str, properties: &[(String, String)]) -> bool {
    let current_rules = normalized_horizontal_size_rules(properties);
    if current_rules.is_empty() {
        return true;
    }

    let Some(stock_rules) = baselines::stock_horizontal_size_rules().get(class_name) else {
        // No shipped baseline means any size rules on that class should stay visible
        return false;
    };

    // Matching every size rule in the current block keeps the shipped theme quiet
    current_rules.iter().all(|(name, value)| {
        stock_rules
            .get(name.as_str())
            .map(|stock_value| stock_value == value)
            .unwrap_or(false)
    })
}

pub(super) fn normalized_horizontal_size_rules(
    properties: &[(String, String)],
) -> HashMap<String, String> {
    let mut current_rules = HashMap::new();
    for (name, value) in properties
        .iter()
        .filter(|(name, _)| super::super::main_css_check_policy::is_horizontal_size_property(name))
    {
        // The same selector can carry a literal fallback and a token override
        // The comparison needs the final value that GTK will keep
        // Duplicate properties use the later value, so the comparison needs the same rule
        current_rules.insert(name.trim().to_string(), value.trim().to_string());
    }
    current_rules
}
