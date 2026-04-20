//! Active theme target discovery and path sanity checks for css-check

mod collect;
mod model;
mod paths;

pub(super) use collect::collect_css_check_inputs;

#[cfg(test)]
pub(super) use collect::collect_css_check_inputs_from;

#[cfg(test)]
mod tests;
