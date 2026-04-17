//! Media-specific geometry math and card-height heuristics for css-check
//!
//! Splits shell modeling, width math, height math, and tests into focused files

mod height;
mod helpers;
mod shell;
mod vertical;
mod width;

#[cfg(test)]
mod tests;

// The geometry model stores the vertical media box state directly
pub(super) use self::vertical::MediaVerticalModel;
