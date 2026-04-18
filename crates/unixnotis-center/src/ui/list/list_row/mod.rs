//! Row-specific widget builders and update helpers for the list view
//!
//! The list keeps state and indexing at the top level
//! Each row kind stays in its own file under this folder

pub(super) mod empty;
pub(super) mod ghost;
pub(super) mod group;
pub(super) mod notification;
