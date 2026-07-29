//! Ordered application attribution from process, portal, and desktop evidence

mod candidates;
mod diagnostics;
mod evidence;
mod model;
mod pipeline;
mod resolution;
mod sender_context;
mod validation;

pub(in crate::daemon) use model::{AppClaim, AttributionResolution};
pub(in crate::daemon) use pipeline::resolve_attribution;
pub(in crate::daemon) use resolution::unknown_reply_denied;

#[cfg(test)]
mod tests;
