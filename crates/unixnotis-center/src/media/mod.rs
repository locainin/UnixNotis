//! MPRIS discovery, state tracking, and control

mod api;
mod art;
mod mpris;
mod runtime;

pub use api::{
    start_media_task, MediaArtKey, MediaArtSource, MediaCommand, MediaHandle, MediaInfo,
};
pub use art::{is_public_ip, remote_https_url_allowed};
