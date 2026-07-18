//! Artwork source identity, normalization, and network policy

mod network_policy;
mod source;

pub use network_policy::{is_public_ip, remote_https_url_allowed};
pub(in crate::media) use source::normalize_art_source;
pub use source::{MediaArtKey, MediaArtSource};
