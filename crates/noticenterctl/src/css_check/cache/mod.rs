//! Cache-aware GTK parse stage for css-check

mod dependencies;
mod model;
mod parse;
mod session;
mod store;

pub(super) use session::validate_css_parse_files;

#[cfg(test)]
mod tests;
