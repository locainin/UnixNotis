//! Report model and rendering for css-check

// Report types stay separate from terminal styling so plain and colored output share one model
mod model;
// Rendering stays in one place so section order does not drift across call sites
mod render;
// ANSI styling rules live here so the renderer can stay mostly text-focused
mod style;

pub(super) use model::{CssCheckActiveFile, CssCheckCategory, CssCheckDiagnostic, CssCheckReport};
pub(super) use render::render_css_check_report_for_stdout;

#[cfg(test)]
mod tests;
