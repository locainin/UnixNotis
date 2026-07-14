//! Shared filesystem operations with stable directory anchors

mod atomic;

pub use atomic::{make_file_executable, write_file_atomic, write_file_if_missing};
