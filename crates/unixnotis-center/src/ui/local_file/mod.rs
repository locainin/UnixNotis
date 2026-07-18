//! Shared local-file reading for UI image sources

mod read;

pub(in crate::ui) use read::read_regular_file;
