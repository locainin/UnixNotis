//! Shared filesystem operations with stable directory anchors

mod atomic;

pub use atomic::{write_file_atomic, write_file_if_missing};
