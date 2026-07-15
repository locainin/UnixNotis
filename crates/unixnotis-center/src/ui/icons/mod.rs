//! Notification icon resolution, caching, and background decoding

mod cache;
mod decode;
mod missing;
mod resolution;
mod resolver;
mod theme;
mod types;
mod updates;

pub use resolver::IconResolver;
