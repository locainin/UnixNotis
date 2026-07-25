//! Daemon-owned application association from process and desktop metadata

mod desktop_index;
mod executable;
mod policy;
mod resolver;

pub(in crate::daemon) use desktop_index::DesktopIdentityIndex;
pub(in crate::daemon) use executable::{executable_evidence_for_pid, FileIdentity};
pub(in crate::daemon) use resolver::{resolve_attribution, AppClaim};
