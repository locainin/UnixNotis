//! Preset-facing names for the shared CSS reference scanner

pub use unixnotis_core::{
    collect_css_import_dependency_values as collect_import_dependency_values, CssImportReference,
};
pub(super) use unixnotis_core::{
    collect_css_import_values as collect_import_values, collect_css_url_spans as collect_url_spans,
    collect_css_url_values as collect_url_values,
};

#[cfg(test)]
#[path = "tests/parse.rs"]
mod tests;
