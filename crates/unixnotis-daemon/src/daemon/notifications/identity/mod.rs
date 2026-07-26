//! Daemon-owned application association from process and desktop metadata

mod desktop_index;
mod executable;
mod policy;
mod resolver;
mod sender;
mod sender_cache;

pub use desktop_index::{spawn_desktop_index_refresh, DesktopIdentityIndex};
pub(in crate::daemon) use executable::{executable_evidence_for_pid, FileIdentity};
pub(in crate::daemon) use resolver::{resolve_attribution, unknown_reply_denied, AppClaim};
pub(in crate::daemon) use sender::resolve_sender_metadata;
pub(super) use sender::SenderMetadata;
pub(in crate::daemon) use sender_cache::SenderMetadataCache;
