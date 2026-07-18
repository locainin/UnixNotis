//! MPRIS discovery, state tracking, and control

mod bus;
mod cache;
mod event_loop;
mod events;
mod handle;
mod identifiers;
mod metadata;
mod model;
mod policy;
mod runtime;
mod schedule;
mod signals;
mod snapshot;

pub use handle::{start_media_task, MediaHandle};
pub use identifiers::{MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER, MPRIS_PREFIX};
pub use model::{MediaArtSource, MediaCommand, MediaInfo};
pub use policy::{is_public_ip, remote_https_url_allowed};
pub use signals::{MediaRefreshOrigin, MediaSignal};
