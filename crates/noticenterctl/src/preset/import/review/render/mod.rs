//! Terminal-safe rendering for executable preset content

mod content;
mod measure;
mod model;
mod renderer;
mod style;

pub(in crate::preset) use self::model::RenderedExecReview;
pub(in crate::preset) use self::renderer::render_exec_content_review_with_style;
pub(in crate::preset) use self::style::ReviewStyle;

#[cfg(test)]
mod tests;
